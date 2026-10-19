# Lumen

Live visuals for the beamer. Two windows:

- **Control** (your MacBook): draw with the mouse/trackpad, play with layers and effects.
- **Output** (the beamer): the clean picture, nothing else on it.

## Run

```bash
npm install     # first time only
npm start
```

Or double-click `start.command`.

The output opens on the first non-primary display it finds (your beamer) as a borderless fullscreen window.
Press **Esc** on the output window to close it. Pick another screen, or a draggable window, in **Output → Show on**.

No Electron? `npm run web` serves the same app on <http://127.0.0.1:5177/control.html>; use *Open output*,
drag the pop-up to the beamer and double-click it for fullscreen.

## Using it

**Drawing** – drag on the preview. Pen / Neon / Soft brushes, 12 colours + custom + rainbow.
*Vanish* makes strokes fade away after a few seconds; *Never* keeps them until you clear.
With the kaleidoscope on, you draw on the picture you see and the mirrored copies appear around the centre.
The **Pointer** tool moves things (balls, sparks) without leaving paint.

**FX tab**
- **Infinite zoom** – the main flow. *Flow* pushes the picture endlessly in (or out), *Depth* sets how far apart the
  nested copies are, *Spiral* adds a twist per octave.
- **Kaleidoscope** – segments, *Segment spin* (turns the picture inside each segment; the mirror lines stay put),
  *Pattern angle* (turns the mirror lines), magnify, centre.
- **Warp** – liquid ripple through the picture.
- **Look / Trails / Post** – blur, glow, saturation, hue, brightness, contrast, echo trails, RGB split, vignette.
- Double-click a slider to reset it. *Surprise me* randomises the look.

**LFO and music** – the wave button at the right of every slider (FX and layer settings) lets that control move by
itself. Pick the source at the top of its drawer:
- **LFO** – a slow wave. Pick a shape (sine, triangle, saw, pulse, drift), then turn *Speed* and *Intensity*.
- **Bass · Mid · High · Beat** – the music (see *Audio* below). *Intensity* is how far the music may push the slider
  (100 % = across its whole range), *Smooth* is how slowly it lets go again, *↓ Duck* pulls the slider down with
  the music instead of pushing it up.

The marker on the slider shows where it is right now (green for an LFO, pink for music). The knobs at the top of the FX
tab scale *all* LFOs (Speed, Intensity) and *all* music reactions (Audio) at once; **L** switches every LFO off.
*Randomize* hands a handful of effects their own LFO, and while audio is listening also a few music reactions.

**Layers tab** – stack of layers, each with opacity, blend mode and blur:
- **Floating balls** (repel / attract / swirl / stir on mouse)
- **Plasma** and **Gradient** (linear, radial, conic)
- **Mouse sparks** (burst particles with gravity)
- **Drawing** layers (Pen, Neon, Soft brushes)
- **Image layer & Media Library** – import images via file picker or drag-and-drop (PNG, JPG, WebP, SVG, GIF) with persistent local storage. Built-in vector visual library (Sacred Geometry, Stargate, Cyber Skull, Cosmic Eye, Galaxy, Bioluminescent Radiolaria). Real-time image effects: framing (Fit, Cover, Stretch, Tile, Center), zoom, continuous spin, pan drift, invert, hue tint, saturation, contrast, brightness, retro pixelate / mosaic, threshold stencil, glitch slicing, and mirror symmetry.
- **Intelligent Life layer** – background artificial life simulator featuring autonomous schooling boids (bioluminescent fish, amoeba cells, or wispy spores). Features emergent flocking (separation, alignment, cohesion), nutrient/energy foraging with glowing bursts, interactive curiosity/flee/orbit reactions to mouse and music beat, engineered to run ultra-cheap on CPU (< 0.2 ms per frame).
Add as many as you like; the dropdown under the preview chooses where paint goes. Layer sliders have LFOs too.

**Scenes tab** – seven starting looks plus your own saved ones (effects, LFOs, music reactions, layers; drawings are not
saved). The built-in scenes already carry a few Bass / Mid / High / Beat reactions: they do nothing until you switch
audio on.
Changing scene dissolves into the next one (*Dissolve* sets how long). *Auto-cycle* steps through the scenes by
itself every N seconds.

**Audio tab** – lets the music drive the visuals.
- **Audio input** switch (or **A**) starts listening. *Source* picks the input: the system default, any microphone or
  audio interface, or *Demo beat*, a built-in test loop that needs no input at all (handy to try things out).
- **Gain** sets the input level; **Auto level** (on by default) keeps every band lively whatever the volume, so the
  sliders pump just as hard on a quiet room mic as on a hot line signal. Switch it off to set the level by hand with Gain.
- **Levels** shows what Lumen hears: *Input* (keep it out of the red), then the bands the sliders see: *Bass* (30–150 Hz),
  *Mid* (150 Hz–2 kHz), *High* (2–12 kHz) and *Beat* (a pulse on every kick or bass onset). The same bars sit next to the
  Audio knob on the FX tab, and the top bar shows an *Audio* light that blinks with the beat.
- **React to music** hands a handful of effects a Bass / Mid / High / Beat reaction; *Clear* removes them again.
- Your input, gain and every reaction are remembered, they survive scene changes, and listening resumes by itself at
  the next start.

**Output tab** – status, display picker, resolution, aspect, and a test pattern to line the picture up.
The default is 720p, which is smooth and light on the GPU; use 1080p if your machine has headroom
(the picture is soft and blurry by nature, so the difference is small).

### Music and the microphone

Lumen only needs to *hear* the music: a microphone in the room, or better, a line from the mixer or DJ gear through an
audio interface. The beamer window never listens; it only receives the finished values.

**macOS asks for permission the first time.** When you switch Audio on, macOS shows *"Electron would like to access the
microphone"* (when you start through Terminal or `start.command` it may name *Terminal* instead). Click **OK**.
If you clicked *Don't Allow* (or the Audio tab says the microphone is blocked):

1. Audio tab → **Open System Settings**, or open *System Settings → Privacy & Security → Microphone* yourself.
2. Switch **Electron** (or **Terminal**) on, then switch Audio off and on again in Lumen.
3. Still nothing, or the entry is missing? Reset the question and start again:
   ```bash
   tccutil reset Microphone com.github.Electron
   ```

Only the control window may listen, and only to audio; video and the output window are refused. In `npm run web` mode
the browser asks instead (it works on `127.0.0.1`).

To play music from the same MacBook into Lumen, route it through a loopback device such as BlackHole and pick that as
*Source*. Echo cancellation, noise suppression and the browser's own auto-gain are switched off, so the music arrives
untouched.

Things to know: the picture follows the music with roughly a tenth of a second of delay (analysis, then the link to the
beamer window). The analysis is a few dozen microseconds per frame and allocates nothing, so it costs no frame rate.
If the bars barely move, raise *Gain*; if *Input* is red, lower it at the source.

### Shortcuts

| Key | |
| --- | --- |
| `D` `E` `P` | Draw · Erase · Pointer only |
| `[` `]` | Brush smaller / bigger |
| `C` / `⇧C` | Clear current drawing / all drawings |
| `K` | Kaleidoscope on/off |
| `L` | All LFOs on/off |
| `A` | Audio input on/off |
| `Space` | Pause / resume |
| `B` | Blackout |
| `V` | Preview final / source |
| `O` / `F` | Open-close output / fullscreen |
| `1`–`9` | Recall scene |

## How it works

Both windows load the same page code from a tiny local server (`server.js`, 127.0.0.1 only) and talk over a
`BroadcastChannel`. The control window runs the simulation (ball physics, particles, LFOs, mouse reaction) and streams
a small state packet every frame, plus brush strokes as they are drawn. Audio is captured and analysed in the control
window too, so the packet already holds the final, music-driven values. The output window renders those itself at its
own resolution through a WebGL2 pipeline (`renderer/js/pipeline.js`):
echo → blur pyramid → blur/glow mix → kaleidoscope / infinite zoom / warp + colour → screen.

```
main.js / preload.js      Electron shell: windows, display picker, microphone permission
server.js                 static server for renderer/
renderer/js/layers.js     balls, plasma, gradient, sparks, drawing, image, intelligent life
renderer/js/media.js      built-in vector media library, image import & IndexedDB caching
renderer/js/scene.js      layer compositing, dissolve, kaleidoscope hit-mapping for the mouse
renderer/js/pipeline.js   WebGL2 effects
renderer/js/fx.js         effect list and smoothing / accumulators
renderer/js/mod.js        LFO waves, music sources and stepping
renderer/js/audio.js      audio input: capture, band levels, beat detection
renderer/js/control.js    control UI, input, link to output
renderer/js/output.js     output window
```
