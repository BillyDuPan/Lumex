/**
 * Lumex Pro Project Management System
 * 
 * Provides Adobe-grade self-contained project packaging (.lumex bundles),
 * "Collect All and Save" asset consolidation, auto-saving, dirty state tracking,
 * and startup onboarding / welcome hub management.
 */

import { uid } from './util.js';
import { getAllMedia, registerAsset } from './media.js';
import { BUILTIN_PRESETS } from './presets.js';

export const LUMEX_PROJECT_VERSION = '2.0.0';
const RECENT_KEY = 'lumex.recent_projects.v1';
const AUTOSAVE_KEY = 'lumex.autosave.v1';
const PREF_WELCOME_KEY = 'lumex.pref.show_welcome.v1';

export class ProjectManager {
  constructor() {
    this.currentProject = {
      id: uid(),
      name: 'Showcase Project',
      version: LUMEX_PROJECT_VERSION,
      created: Date.now(),
      modified: Date.now(),
      filePath: null,
      isDemo: true,
    };
    this.isDirty = false;
    this.dirtyListeners = new Set();
    this.autosaveInterval = null;
    this.initAutosave();
  }

  onDirtyChange(fn) {
    this.dirtyListeners.add(fn);
  }

  markDirty() {
    if (!this.isDirty) {
      this.isDirty = true;
      this.notifyDirty();
    }
  }

  markClean() {
    if (this.isDirty) {
      this.isDirty = false;
      this.notifyDirty();
    }
  }

  notifyDirty() {
    for (const fn of this.dirtyListeners) {
      try { fn(this.isDirty, this.currentProject); } catch (e) { console.error(e); }
    }
  }

  shouldShowWelcomeOnStartup() {
    const val = localStorage.getItem(PREF_WELCOME_KEY);
    return val === null ? true : val === 'true';
  }

  setShowWelcomeOnStartup(show) {
    localStorage.setItem(PREF_WELCOME_KEY, String(show));
  }

  getRecentProjects() {
    try {
      return JSON.parse(localStorage.getItem(RECENT_KEY)) || [];
    } catch {
      return [];
    }
  }

  recordRecentProject(meta) {
    let list = this.getRecentProjects().filter((p) => p.name !== meta.name && (!meta.filePath || p.filePath !== meta.filePath));
    list.unshift({
      id: meta.id || uid(),
      name: meta.name || 'Untitled Project',
      modified: meta.modified || Date.now(),
      sceneCount: meta.sceneCount || 0,
      thumbnail: meta.thumbnail || null,
      filePath: meta.filePath || null,
      isDemo: !!meta.isDemo,
    });
    if (list.length > 10) list = list.slice(0, 10);
    try {
      localStorage.setItem(RECENT_KEY, JSON.stringify(list));
    } catch { /* storage limits */ }
  }

  initAutosave(getSnapshotFn) {
    if (this.autosaveInterval) clearInterval(this.autosaveInterval);
    // Autosave snapshot every 3 minutes
    this.autosaveInterval = setInterval(() => {
      if (this.isDirty && typeof getSnapshotFn === 'function') {
        try {
          const snapshot = getSnapshotFn();
          if (snapshot) {
            localStorage.setItem(AUTOSAVE_KEY, JSON.stringify({
              timestamp: Date.now(),
              projectName: this.currentProject.name,
              data: snapshot,
            }));
            console.log(`[ProjectManager] Autosave snapshot captured at ${new Date().toLocaleTimeString()}`);
          }
        } catch (e) {
          console.warn('[ProjectManager] Autosave failed:', e);
        }
      }
    }, 180000);
  }

  hasAutosave() {
    try {
      const data = localStorage.getItem(AUTOSAVE_KEY);
      return !!data;
    } catch {
      return false;
    }
  }

  getAutosave() {
    try {
      return JSON.parse(localStorage.getItem(AUTOSAVE_KEY));
    } catch {
      return null;
    }
  }

  clearAutosave() {
    localStorage.removeItem(AUTOSAVE_KEY);
  }

  /**
   * Package current application state into a self-contained .lumex bundle.
   * Consolidates all user images and assets so projects never have broken links.
   */
  packageProject(appState, thumbnailDataUrl) {
    const { user: userMedia } = getAllMedia();
    
    // Bundle all user media assets
    const assets = (userMedia || []).map((m) => ({
      id: m.id,
      name: m.name,
      category: m.category || 'user',
      dataUrl: m.dataUrl,
      thumb: m.thumb || m.dataUrl,
      width: m.width || 1280,
      height: m.height || 720,
      date: m.date || Date.now(),
    }));

    const projectPackage = {
      lumexVersion: LUMEX_PROJECT_VERSION,
      id: this.currentProject.id || uid(),
      name: this.currentProject.name || 'Untitled Project',
      created: this.currentProject.created || Date.now(),
      modified: Date.now(),
      thumbnail: thumbnailDataUrl || null,
      settings: appState.settings || {},
      clock: appState.clock || {},
      brush: appState.brush || {},
      audioCfg: appState.audioCfg || {},
      lfo: appState.lfo || {},
      decks: appState.decks || {
        deckA: { sceneId: appState.activeSceneId, name: 'Deck A' },
        deckB: { sceneId: null, name: 'Deck B' },
        crossfader: 0.0,
        curve: 'smooth',
      },
      scenes: appState.scenes || [],
      activeSceneId: appState.activeSceneId || null,
      config: appState.config || null,
      fx: appState.fx || null,
      fxMods: appState.fxMods || null,
      assets,
    };

    return projectPackage;
  }

  /**
   * Unpack a .lumex project package string and register all bundled assets.
   */
  unpackageProject(jsonString) {
    let pkg;
    try {
      pkg = typeof jsonString === 'string' ? JSON.parse(jsonString) : jsonString;
    } catch (err) {
      throw new Error(`Invalid Lumex project file: ${err.message}`);
    }

    if (!pkg.lumexVersion && !pkg.scenes) {
      throw new Error('Unrecognized project format. Expected a Lumex Pro project bundle.');
    }

    // Register all bundled media assets into media cache
    if (Array.isArray(pkg.assets)) {
      for (const asset of pkg.assets) {
        if (asset.id && asset.dataUrl) {
          registerAsset(asset.id, asset.dataUrl, asset.name);
        }
      }
    }

    this.currentProject = {
      id: pkg.id || uid(),
      name: pkg.name || 'Imported Project',
      version: pkg.lumexVersion || LUMEX_PROJECT_VERSION,
      created: pkg.created || Date.now(),
      modified: pkg.modified || Date.now(),
      filePath: pkg.filePath || null,
      isDemo: false,
    };

    this.markClean();
    return pkg;
  }

  /**
   * Export project package to disk or trigger download.
   */
  async saveToFile(projectPackage, suggestedFilename) {
    const filename = suggestedFilename || `${projectPackage.name.replace(/[^a-zA-Z0-9_-]/g, '_')}.lumex`;
    const jsonString = JSON.stringify(projectPackage, null, 2);

    // Try Tauri native command if available
    if (typeof window !== 'undefined' && window.__TAURI__?.core) {
      try {
        if (this.currentProject.filePath) {
          await window.__TAURI__.core.invoke('save_project_file', {
            path: this.currentProject.filePath,
            contents: jsonString,
          });
          this.markClean();
          this.recordRecentProject({
            ...this.currentProject,
            sceneCount: projectPackage.scenes?.length || 0,
            thumbnail: projectPackage.thumbnail,
          });
          return { success: true, path: this.currentProject.filePath };
        }
      } catch (err) {
        console.warn('[ProjectManager] Native save failed, falling back to download:', err);
      }
    }

    // Browser Blob download fallback
    const blob = new Blob([jsonString], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);

    this.markClean();
    this.recordRecentProject({
      ...this.currentProject,
      sceneCount: projectPackage.scenes?.length || 0,
      thumbnail: projectPackage.thumbnail,
    });
    return { success: true, filename };
  }

  /**
   * Create a clean new blank project.
   */
  createNewProject(name = 'Untitled Project', aspect = 16 / 9, bpm = 124.0) {
    this.currentProject = {
      id: uid(),
      name,
      version: LUMEX_PROJECT_VERSION,
      created: Date.now(),
      modified: Date.now(),
      filePath: null,
      isDemo: false,
    };
    this.markClean();

    return {
      name,
      settings: { aspect, aspectMode: '16:9', outH: 720 },
      clock: { bpm, source: 'tap', loopLength: 32 },
      scenes: [],
      config: { bg: '#020108', layers: [] },
      decks: {
        deckA: { sceneId: null, name: 'Deck A' },
        deckB: { sceneId: null, name: 'Deck B' },
        crossfader: 0.0,
        curve: 'smooth',
      },
    };
  }

  /**
   * Load the built-in Showcase Project with the 7 pre-made scenes.
   */
  createShowcaseProject() {
    this.currentProject = {
      id: 'showcase_default',
      name: 'Showcase Project',
      version: LUMEX_PROJECT_VERSION,
      created: Date.now(),
      modified: Date.now(),
      filePath: null,
      isDemo: true,
    };
    this.markClean();

    return {
      name: 'Showcase Project',
      settings: { aspect: 16 / 9, aspectMode: 'auto', outH: 720, glide: 2.0 },
      clock: { bpm: 124.0, source: 'tap', loopLength: 32 },
      scenes: BUILTIN_PRESETS.map((p) => ({ ...p, user: false })),
      activeSceneId: BUILTIN_PRESETS[0].id,
      decks: {
        deckA: { sceneId: BUILTIN_PRESETS[0].id, name: BUILTIN_PRESETS[0].name },
        deckB: { sceneId: BUILTIN_PRESETS[1]?.id || null, name: BUILTIN_PRESETS[1]?.name || 'Deck B' },
        crossfader: 0.0,
        curve: 'smooth',
      },
    };
  }
}

export const projectMgr = new ProjectManager();
