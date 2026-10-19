// Media library: built-in vector visual assets and custom imported image storage.
import { uid } from './util.js';

const DB_NAME = 'lumen_media_v1';
const STORE_NAME = 'assets';

// Built-in vector visual artworks (sharp at 4K, transparent, zero bandwidth)
const BUILTINS = [
  {
    id: 'builtin:mandala_flower',
    name: 'Flower of Life',
    category: 'sacred',
    svg: `<svg viewBox="0 0 500 500" fill="none" stroke="#ffffff" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
      <circle cx="250" cy="250" r="236" stroke-width="4" opacity="0.9"/>
      <circle cx="250" cy="250" r="226" stroke-width="1.5" opacity="0.6"/>
      <circle cx="250" cy="250" r="70"/>
      <!-- Ring 1 (6 circles) -->
      <circle cx="250" cy="180" r="70"/>
      <circle cx="310.6" cy="215" r="70"/>
      <circle cx="310.6" cy="285" r="70"/>
      <circle cx="250" cy="320" r="70"/>
      <circle cx="189.4" cy="285" r="70"/>
      <circle cx="189.4" cy="215" r="70"/>
      <!-- Ring 2 (12 circles) -->
      <circle cx="250" cy="110" r="70"/>
      <circle cx="310.6" cy="145" r="70"/>
      <circle cx="371.2" cy="180" r="70"/>
      <circle cx="371.2" cy="250" r="70"/>
      <circle cx="371.2" cy="320" r="70"/>
      <circle cx="310.6" cy="355" r="70"/>
      <circle cx="250" cy="390" r="70"/>
      <circle cx="189.4" cy="355" r="70"/>
      <circle cx="128.8" cy="320" r="70"/>
      <circle cx="128.8" cy="250" r="70"/>
      <circle cx="128.8" cy="180" r="70"/>
      <circle cx="189.4" cy="145" r="70"/>
      <!-- Outer petal arcs -->
      <circle cx="250" cy="250" r="140" stroke-width="1.5" stroke-dasharray="4 6" opacity="0.5"/>
    </svg>`,
  },
  {
    id: 'builtin:sacred_metatron',
    name: "Metatron's Cube",
    category: 'sacred',
    svg: `<svg viewBox="0 0 500 500" fill="none" stroke="#ffffff" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <circle cx="250" cy="250" r="238" stroke-width="3" opacity="0.8"/>
      <!-- Inner and outer nodes -->
      <g stroke-width="1.8" opacity="0.85">
        <circle cx="250" cy="250" r="28" fill="#ffffff" fill-opacity="0.1"/>
        <circle cx="250" cy="160" r="28"/>
        <circle cx="328" cy="205" r="28"/>
        <circle cx="328" cy="295" r="28"/>
        <circle cx="250" cy="340" r="28"/>
        <circle cx="172" cy="295" r="28"/>
        <circle cx="172" cy="205" r="28"/>
        <circle cx="250" cy="70" r="28"/>
        <circle cx="406" cy="160" r="28"/>
        <circle cx="406" cy="340" r="28"/>
        <circle cx="250" cy="430" r="28"/>
        <circle cx="94" cy="340" r="28"/>
        <circle cx="94" cy="160" r="28"/>
      </g>
      <!-- Metatron interconnection lines -->
      <g stroke-width="1.2" opacity="0.65">
        <!-- Center to all nodes -->
        <line x1="250" y1="250" x2="250" y2="70"/><line x1="250" y1="250" x2="406" y2="160"/>
        <line x1="250" y1="250" x2="406" y2="340"/><line x1="250" y1="250" x2="250" y2="430"/>
        <line x1="250" y1="250" x2="94" y2="340"/><line x1="250" y1="250" x2="94" y2="160"/>
        <!-- Star of David large -->
        <polygon points="250,70 406,340 94,340" stroke-width="2.2" opacity="0.9"/>
        <polygon points="250,430 406,160 94,160" stroke-width="2.2" opacity="0.9"/>
        <!-- Hexagon outer -->
        <polygon points="250,70 406,160 406,340 250,430 94,340 94,160" stroke-width="1.8"/>
        <!-- Hexagram inner -->
        <polygon points="250,160 328,295 172,295" stroke-width="1.8"/>
        <polygon points="250,340 328,205 172,205" stroke-width="1.8"/>
        <!-- Cross connections -->
        <line x1="250" y1="70" x2="250" y2="430"/><line x1="94" y1="160" x2="406" y2="340"/>
        <line x1="94" y1="340" x2="406" y2="160"/><line x1="172" y1="205" x2="328" y2="295"/>
        <line x1="172" y1="295" x2="328" y2="205"/><line x1="250" y1="160" x2="250" y2="340"/>
      </g>
    </svg>`,
  },
  {
    id: 'builtin:sacred_sri',
    name: 'Sri Yantra Star',
    category: 'sacred',
    svg: `<svg viewBox="0 0 500 500" fill="none" stroke="#ffffff" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
      <circle cx="250" cy="250" r="235" stroke-width="3" opacity="0.8"/>
      <circle cx="250" cy="250" r="222" stroke-width="1" stroke-dasharray="3 5"/>
      <circle cx="250" cy="250" r="185" stroke-width="2"/>
      <circle cx="250" cy="250" r="150" stroke-width="1.5"/>
      <!-- Interlocking Triangles -->
      <polygon points="250,75 390,345 110,345" stroke-width="2.2"/>
      <polygon points="250,425 390,155 110,155" stroke-width="2.2"/>
      <polygon points="250,110 365,325 135,325" stroke-width="1.8"/>
      <polygon points="250,390 365,175 135,175" stroke-width="1.8"/>
      <polygon points="250,145 340,305 160,305" stroke-width="1.5"/>
      <polygon points="250,355 340,195 160,195" stroke-width="1.5"/>
      <polygon points="250,180 315,285 185,285" stroke-width="1.4"/>
      <polygon points="250,320 315,215 185,215" stroke-width="1.4"/>
      <polygon points="250,210 290,270 210,270" stroke-width="1.2"/>
      <!-- Bindu Center -->
      <circle cx="250" cy="250" r="5" fill="#ffffff"/>
      <!-- Radiating lotus points around ring -->
      <circle cx="250" cy="250" r="204" stroke-width="1" stroke-dasharray="6 14"/>
    </svg>`,
  },
  {
    id: 'builtin:zeno_lotus',
    name: 'Sacred Lotus',
    category: 'sacred',
    svg: `<svg viewBox="0 0 500 500" fill="none" stroke="#ffffff" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <circle cx="250" cy="250" r="236" stroke-width="3" opacity="0.6"/>
      <circle cx="250" cy="250" r="28" fill="#ffffff" fill-opacity="0.2"/>
      <circle cx="250" cy="250" r="8" fill="#ffffff"/>
      <!-- 8-fold and 16-fold Lotus Petals -->
      <g stroke-width="1.8">
        <path d="M250,222 C230,170 210,100 250,40 C290,100 270,170 250,222 Z"/>
        <path d="M250,278 C230,330 210,400 250,460 C290,400 270,330 250,278 Z"/>
        <path d="M222,250 C170,230 100,210 40,250 C100,290 170,270 222,250 Z"/>
        <path d="M278,250 C330,230 400,210 460,250 C400,290 330,270 278,250 Z"/>
        <path d="M230,230 C185,190 120,135 102,102 C135,120 190,185 230,230 Z"/>
        <path d="M270,230 C315,190 380,135 398,102 C365,120 310,185 270,230 Z"/>
        <path d="M230,270 C185,310 120,365 102,398 C135,380 190,315 230,270 Z"/>
        <path d="M270,270 C315,310 380,365 398,398 C365,380 310,315 270,270 Z"/>
      </g>
      <!-- Secondary layer rotated -->
      <g stroke-width="1.2" opacity="0.75" transform="rotate(22.5 250 250)">
        <path d="M250,222 C235,180 220,120 250,70 C280,120 265,180 250,222 Z"/>
        <path d="M250,278 C235,320 220,380 250,430 C280,380 265,320 250,278 Z"/>
        <path d="M222,250 C180,235 120,220 70,250 C120,280 180,265 222,250 Z"/>
        <path d="M278,250 C320,235 380,220 430,250 C380,280 320,265 278,250 Z"/>
        <path d="M230,230 C195,200 145,155 125,125 C155,145 200,195 230,230 Z"/>
        <path d="M270,230 C305,200 355,155 375,125 C345,145 300,195 270,230 Z"/>
        <path d="M230,270 C195,300 145,345 125,375 C155,355 200,305 230,270 Z"/>
        <path d="M270,270 C305,300 355,345 375,375 C345,355 300,305 270,270 Z"/>
      </g>
      <circle cx="250" cy="250" r="180" stroke-dasharray="2 8" opacity="0.6"/>
    </svg>`,
  },
  {
    id: 'builtin:portal_stargate',
    name: 'Stargate Portal',
    category: 'cosmic',
    svg: `<svg viewBox="0 0 500 500" fill="none" stroke="#ffffff" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <circle cx="250" cy="250" r="235" stroke-width="4"/>
      <circle cx="250" cy="250" r="222" stroke-width="1.5" stroke-dasharray="4 4"/>
      <circle cx="250" cy="250" r="195" stroke-width="3"/>
      <circle cx="250" cy="250" r="160" stroke-width="1.5" stroke-dasharray="8 6"/>
      <circle cx="250" cy="250" r="130" stroke-width="2"/>
      <circle cx="250" cy="250" r="90" stroke-width="2.5" stroke-dasharray="12 10"/>
      <circle cx="250" cy="250" r="50" stroke-width="2"/>
      <circle cx="250" cy="250" r="20" fill="#ffffff" fill-opacity="0.3"/>
      <!-- Chevrons / Glyph nodes -->
      <g stroke-width="2.5">
        <polygon points="250,22 240,45 260,45" fill="#ffffff"/>
        <polygon points="250,478 240,455 260,455" fill="#ffffff"/>
        <polygon points="22,250 45,240 45,260" fill="#ffffff"/>
        <polygon points="478,250 455,240 455,260" fill="#ffffff"/>
        <polygon points="89,89 105,108 116,97" fill="#ffffff"/>
        <polygon points="411,89 395,108 384,97" fill="#ffffff"/>
        <polygon points="89,411 105,392 116,403" fill="#ffffff"/>
        <polygon points="411,411 395,392 384,403" fill="#ffffff"/>
      </g>
      <!-- Radiating portal rays -->
      <g stroke-width="1" opacity="0.6">
        <line x1="250" y1="50" x2="250" y2="130"/><line x1="250" y1="370" x2="250" y2="450"/>
        <line x1="50" y1="250" x2="130" y2="250"/><line x1="370" y1="250" x2="450" y2="250"/>
        <line x1="109" y1="109" x2="165" y2="165"/><line x1="335" y1="335" x2="391" y2="391"/>
        <line x1="109" y1="391" x2="165" y2="335"/><line x1="335" y1="165" x2="391" y2="109"/>
      </g>
    </svg>`,
  },
  {
    id: 'builtin:cyber_skull',
    name: 'Cyber Skull',
    category: 'cosmic',
    svg: `<svg viewBox="0 0 500 500" fill="none" stroke="#ffffff" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
      <circle cx="250" cy="250" r="236" stroke-width="2" stroke-dasharray="6 8" opacity="0.4"/>
      <!-- Skull Cranium Wireframe -->
      <path d="M140,240 C125,180 145,100 210,75 C230,68 270,68 290,75 C355,100 375,180 360,240 C355,270 335,285 320,305 L310,370 L190,370 L180,305 C165,285 145,270 140,240 Z" stroke-width="3"/>
      <!-- Forehead Crest Lines -->
      <line x1="250" y1="72" x2="250" y2="210" stroke-width="1.5" stroke-dasharray="4 4"/>
      <path d="M190,130 L250,165 L310,130"/>
      <path d="M170,175 L250,215 L330,175"/>
      <!-- Eye Sockets -->
      <polygon points="175,220 225,230 215,280 165,270" stroke-width="2.5" fill="#ffffff" fill-opacity="0.15"/>
      <polygon points="325,220 275,230 285,280 335,270" stroke-width="2.5" fill="#ffffff" fill-opacity="0.15"/>
      <!-- Nose Cavity -->
      <polygon points="250,280 238,315 262,315" fill="#ffffff"/>
      <!-- Teeth / Jaw Grid -->
      <path d="M200,340 L300,340"/>
      <line x1="220" y1="330" x2="220" y2="365"/><line x1="240" y1="330" x2="240" y2="365"/>
      <line x1="260" y1="330" x2="260" y2="365"/><line x1="280" y1="330" x2="280" y2="365"/>
      <!-- Cyber Circuit Tracks -->
      <path d="M140,200 L90,200 L70,240" stroke-width="1.8"/>
      <circle cx="70" cy="240" r="4" fill="#ffffff"/>
      <path d="M360,200 L410,200 L430,240" stroke-width="1.8"/>
      <circle cx="430" cy="240" r="4" fill="#ffffff"/>
      <path d="M180,370 L150,420 L90,420" stroke-width="1.8"/>
      <circle cx="90" cy="420" r="4" fill="#ffffff"/>
      <path d="M320,370 L350,420 L410,420" stroke-width="1.8"/>
      <circle cx="410" cy="420" r="4" fill="#ffffff"/>
    </svg>`,
  },
  {
    id: 'builtin:celestial_eye',
    name: 'Cosmic Eye',
    category: 'cosmic',
    svg: `<svg viewBox="0 0 500 500" fill="none" stroke="#ffffff" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <circle cx="250" cy="250" r="236" stroke-width="2.5" opacity="0.6"/>
      <!-- Radiating Sunbeams (16 rays) -->
      <g stroke-width="1.8" opacity="0.75">
        <line x1="250" y1="20" x2="250" y2="90"/><line x1="250" y1="410" x2="250" y2="480"/>
        <line x1="20" y1="250" x2="90" y2="250"/><line x1="410" y1="250" x2="480" y2="250"/>
        <line x1="87" y1="87" x2="137" y2="137"/><line x1="363" y1="363" x2="413" y2="413"/>
        <line x1="87" y1="413" x2="137" y2="363"/><line x1="363" y1="137" x2="413" y2="87"/>
        <line x1="155" y1="45" x2="185" y2="110"/><line x1="345" y1="455" x2="315" y2="390"/>
        <line x1="45" y1="155" x2="110" y2="185"/><line x1="455" y1="345" x2="390" y2="315"/>
        <line x1="345" y1="45" x2="315" y2="110"/><line x1="155" y1="455" x2="185" y2="390"/>
        <line x1="455" y1="155" x2="390" y2="185"/><line x1="45" y1="345" x2="110" y2="315"/>
      </g>
      <!-- All Seeing Eye Lids -->
      <path d="M70,250 C140,140 360,140 430,250 C360,360 140,360 70,250 Z" stroke-width="3.5" fill="#ffffff" fill-opacity="0.05"/>
      <!-- Inner Eyelid Curve -->
      <path d="M100,250 C160,165 340,165 400,250 C340,335 160,335 100,250 Z" stroke-width="1.8"/>
      <!-- Iris & Pupil -->
      <circle cx="250" cy="250" r="64" stroke-width="3"/>
      <circle cx="250" cy="250" r="46" stroke-width="1.5" stroke-dasharray="3 4"/>
      <circle cx="250" cy="250" r="26" fill="#ffffff"/>
      <circle cx="260" cy="242" r="7" fill="#000000"/>
      <!-- Crescent Moon Crown -->
      <path d="M230,120 A25,25 0 0,0 270,120 A20,20 0 0,1 230,120" fill="#ffffff"/>
    </svg>`,
  },
  {
    id: 'builtin:galaxy_vortex',
    name: 'Galaxy Vortex',
    category: 'cosmic',
    svg: `<svg viewBox="0 0 500 500" fill="none" stroke="#ffffff" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <circle cx="250" cy="250" r="236" stroke-width="1.5" stroke-dasharray="3 6" opacity="0.4"/>
      <!-- Central Core -->
      <circle cx="250" cy="250" r="30" stroke-width="3" fill="#ffffff" fill-opacity="0.2"/>
      <circle cx="250" cy="250" r="14" fill="#ffffff"/>
      <!-- 4 Spiral Arms -->
      <g stroke-width="2.5">
        <path d="M250,220 C280,210 320,230 340,260 C365,300 350,360 300,390 C240,425 155,395 125,325 C90,240 135,130 230,95 C340,55 465,115 480,235"/>
        <path d="M250,280 C220,290 180,270 160,240 C135,200 150,140 200,110 C260,75 345,105 375,175 C410,260 365,370 270,405 C160,445 35,385 20,265" opacity="0.9"/>
        <path d="M220,250 C210,220 230,180 260,160 C300,135 360,150 390,200 C425,260 395,345 325,375 C240,410 130,365 95,270 C55,160 115,35 235,20" opacity="0.7" stroke-width="1.8"/>
        <path d="M280,250 C290,280 270,320 240,340 C200,365 140,350 110,300 C75,240 105,155 175,125 C260,90 370,135 405,230 C445,340 385,465 265,480" opacity="0.7" stroke-width="1.8"/>
      </g>
      <!-- Star Clusters -->
      <circle cx="340" cy="180" r="3" fill="#ffffff"/><circle cx="160" cy="320" r="3" fill="#ffffff"/>
      <circle cx="390" cy="300" r="4" fill="#ffffff"/><circle cx="110" cy="200" r="4" fill="#ffffff"/>
      <circle cx="210" cy="130" r="2.5" fill="#ffffff"/><circle cx="290" cy="370" r="2.5" fill="#ffffff"/>
    </svg>`,
  },
  {
    id: 'builtin:synth_horizon',
    name: 'Synth Horizon',
    category: 'cosmic',
    svg: `<svg viewBox="0 0 500 500" fill="none" stroke="#ffffff" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <circle cx="250" cy="250" r="236" stroke-width="2" opacity="0.5"/>
      <!-- Sliced Retro Sun -->
      <g stroke-width="2.5">
        <path d="M150,220 A105,105 0 0,1 350,220 Z" fill="#ffffff" fill-opacity="0.15"/>
        <path d="M152,205 H348"/><path d="M158,185 H342"/>
        <path d="M168,160 H332"/><path d="M182,135 H318"/>
        <path d="M205,110 H295"/><path d="M230,85 H270"/>
      </g>
      <!-- Horizon Line -->
      <line x1="20" y1="240" x2="480" y2="240" stroke-width="4"/>
      <!-- Wireframe Perspective Grid (Road / Ground) -->
      <g stroke-width="1.8">
        <line x1="250" y1="240" x2="250" y2="480"/>
        <line x1="250" y1="240" x2="190" y2="480"/>
        <line x1="250" y1="240" x2="310" y2="480"/>
        <line x1="250" y1="240" x2="110" y2="480"/>
        <line x1="250" y1="240" x2="390" y2="480"/>
        <line x1="250" y1="240" x2="20" y2="480"/>
        <line x1="250" y1="240" x2="480" y2="480"/>
        <!-- Transverse perspective grid steps -->
        <line x1="230" y1="248" x2="270" y2="248"/>
        <line x1="210" y1="258" x2="290" y2="258"/>
        <line x1="185" y1="272" x2="315" y2="272"/>
        <line x1="150" y1="292" x2="350" y2="292"/>
        <line x1="105" y1="322" x2="395" y2="322"/>
        <line x1="55" y1="365" x2="445" y2="365"/>
        <line x1="20" y1="425" x2="480" y2="425"/>
      </g>
    </svg>`,
  },
  {
    id: 'builtin:hypercube',
    name: 'Tesseract Hypercube',
    category: 'sacred',
    svg: `<svg viewBox="0 0 500 500" fill="none" stroke="#ffffff" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round">
      <circle cx="250" cy="250" r="236" stroke-width="1.5" stroke-dasharray="4 8" opacity="0.4"/>
      <!-- Outer Hexagonal / Cube Projection -->
      <polygon points="250,50 420,150 420,350 250,450 80,350 80,150" stroke-width="3"/>
      <!-- Inner Cube Projection -->
      <polygon points="250,150 335,200 335,300 250,350 165,300 165,200" stroke-width="2.5" fill="#ffffff" fill-opacity="0.1"/>
      <!-- Connecting Hypercube Struts -->
      <g stroke-width="2.2" opacity="0.85">
        <line x1="250" y1="50" x2="250" y2="150"/>
        <line x1="420" y1="150" x2="335" y2="200"/>
        <line x1="420" y1="350" x2="335" y2="300"/>
        <line x1="250" y1="450" x2="250" y2="350"/>
        <line x1="80" y1="350" x2="165" y2="300"/>
        <line x1="80" y1="150" x2="165" y2="200"/>
      </g>
      <!-- Isometric Center Axes -->
      <g stroke-width="1.4" opacity="0.6">
        <line x1="250" y1="250" x2="250" y2="150"/><line x1="250" y1="250" x2="335" y2="300"/>
        <line x1="250" y1="250" x2="165" y2="300"/><line x1="250" y1="250" x2="250" y2="350"/>
        <line x1="250" y1="250" x2="335" y2="200"/><line x1="250" y1="250" x2="165" y2="200"/>
      </g>
      <!-- Vertex Nodes -->
      <circle cx="250" cy="50" r="5" fill="#ffffff"/><circle cx="420" cy="150" r="5" fill="#ffffff"/>
      <circle cx="420" cy="350" r="5" fill="#ffffff"/><circle cx="250" cy="450" r="5" fill="#ffffff"/>
      <circle cx="80" cy="350" r="5" fill="#ffffff"/><circle cx="80" cy="150" r="5" fill="#ffffff"/>
      <circle cx="250" cy="250" r="7" fill="#ffffff"/>
    </svg>`,
  },
  {
    id: 'builtin:bio_radiolaria',
    name: 'Bio Radiolaria',
    category: 'organic',
    svg: `<svg viewBox="0 0 500 500" fill="none" stroke="#ffffff" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <!-- 12 Radial Spines -->
      <g stroke-width="2.5">
        <line x1="250" y1="20" x2="250" y2="480"/>
        <line x1="20" y1="250" x2="480" y2="250"/>
        <line x1="87" y1="87" x2="413" y2="413"/>
        <line x1="87" y1="413" x2="413" y2="87"/>
        <line x1="155" y1="45" x2="345" y2="455"/>
        <line x1="45" y1="155" x2="455" y2="345"/>
        <line x1="345" y1="45" x2="155" y2="455"/>
        <line x1="455" y1="155" x2="45" y2="345"/>
      </g>
      <!-- Concentric Porous Shells -->
      <circle cx="250" cy="250" r="190" stroke-width="3"/>
      <circle cx="250" cy="250" r="160" stroke-width="1.8" stroke-dasharray="6 8"/>
      <circle cx="250" cy="250" r="125" stroke-width="2"/>
      <circle cx="250" cy="250" r="90" stroke-width="1.8" stroke-dasharray="4 6"/>
      <circle cx="250" cy="250" r="55" stroke-width="2.5" fill="#ffffff" fill-opacity="0.15"/>
      <circle cx="250" cy="250" r="24" stroke-width="2"/>
      <circle cx="250" cy="250" r="8" fill="#ffffff"/>
      <!-- Siliceous Spine Tips -->
      <polygon points="250,15 244,35 256,35" fill="#ffffff"/>
      <polygon points="250,485 244,465 256,465" fill="#ffffff"/>
      <polygon points="15,250 35,244 35,256" fill="#ffffff"/>
      <polygon points="485,250 465,244 465,256" fill="#ffffff"/>
    </svg>`,
  },
  {
    id: 'builtin:nautilus_spiral',
    name: 'Nautilus Shell',
    category: 'organic',
    svg: `<svg viewBox="0 0 500 500" fill="none" stroke="#ffffff" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
      <circle cx="250" cy="250" r="236" stroke-width="1.5" stroke-dasharray="4 6" opacity="0.4"/>
      <!-- Golden Spiral Chambers -->
      <path d="M250,250 C253,248 255,250 255,253 C255,258 248,262 243,260 C234,257 232,243 238,235 C249,221 270,221 283,235 C301,254 297,284 274,302 C244,326 201,317 178,284 C148,242 163,184 207,153 C263,113 338,134 378,192 C428,264 398,360 322,408 C228,467 108,427 52,328 C-10,215 42,75 160,18" stroke-width="3"/>
      <!-- Chamber Septa Divisors -->
      <g stroke-width="1.6" opacity="0.75">
        <path d="M250,250 L255,258"/><path d="M243,260 L234,248"/><path d="M238,235 L252,225"/>
        <path d="M283,235 L275,255"/><path d="M274,302 L248,290"/><path d="M178,284 L205,255"/>
        <path d="M207,153 L245,200"/><path d="M378,192 L315,225"/><path d="M322,408 L265,320"/>
        <path d="M52,328 L155,280"/><path d="M160,18 L240,165"/>
      </g>
    </svg>`,
  },
];

// In-memory cache of HTMLImageElements and asset descriptors
const imageCache = new Map();
const assetCache = new Map();

// Generate data URL for SVG built-in
for (const b of BUILTINS) {
  const encoded = 'data:image/svg+xml;charset=utf-8,' + encodeURIComponent(b.svg);
  b.dataUrl = encoded;
  b.thumb = encoded;
  assetCache.set(b.id, b);
}

/* ------------------------------------------------------------- IndexedDB */

let dbPromise = null;
function getDb() {
  if (dbPromise) return dbPromise;
  dbPromise = new Promise((resolve) => {
    if (typeof indexedDB === 'undefined') return resolve(null);
    try {
      const req = indexedDB.open(DB_NAME, 1);
      req.onupgradeneeded = (e) => {
        const db = e.target.result;
        if (!db.objectStoreNames.contains(STORE_NAME)) {
          db.createObjectStore(STORE_NAME, { keyPath: 'id' });
        }
      };
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => resolve(null);
    } catch {
      resolve(null);
    }
  });
  return dbPromise;
}

export async function loadUserMedia() {
  const db = await getDb();
  if (!db) return [];
  return new Promise((resolve) => {
    try {
      const tx = db.transaction(STORE_NAME, 'readonly');
      const store = tx.objectStore(STORE_NAME);
      const req = store.getAll();
      req.onsuccess = () => {
        const list = req.result || [];
        for (const it of list) assetCache.set(it.id, it);
        resolve(list);
      };
      req.onerror = () => resolve([]);
    } catch {
      resolve([]);
    }
  });
}

export async function saveUserMedia(file) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = reject;
    reader.onload = async () => {
      const originalUrl = reader.result;
      const img = new Image();
      img.onload = async () => {
        // Downscale if too large (max 1920x1080) for memory and performance
        const maxW = 1920, maxH = 1080;
        let w = img.width, h = img.height;
        if (w > maxW || h > maxH) {
          const sc = Math.min(maxW / w, maxH / h);
          w = Math.round(w * sc);
          h = Math.round(h * sc);
        }
        const c = document.createElement('canvas');
        c.width = w; c.height = h;
        const ctx = c.getContext('2d');
        ctx.drawImage(img, 0, 0, w, h);
        const mime = file.type === 'image/png' ? 'image/png' : 'image/jpeg';
        const dataUrl = c.toDataURL(mime, 0.88);

        // Generate small thumbnail
        const thumbC = document.createElement('canvas');
        thumbC.width = 120; thumbC.height = 120;
        const tctx = thumbC.getContext('2d');
        const minDim = Math.min(img.width, img.height);
        const sx = (img.width - minDim) / 2, sy = (img.height - minDim) / 2;
        tctx.drawImage(img, sx, sy, minDim, minDim, 0, 0, 120, 120);
        const thumb = thumbC.toDataURL('image/jpeg', 0.8);

        const asset = {
          id: 'user:' + uid(),
          name: file.name.replace(/\.[^/.]+$/, ''),
          category: 'user',
          dataUrl,
          thumb,
          width: w,
          height: h,
          date: Date.now(),
        };

        assetCache.set(asset.id, asset);
        const db = await getDb();
        if (db) {
          try {
            const tx = db.transaction(STORE_NAME, 'readwrite');
            tx.objectStore(STORE_NAME).put(asset);
          } catch { /* storage fallback */ }
        }
        resolve(asset);
      };
      img.src = originalUrl;
    };
    reader.readAsDataURL(file);
  });
}

export async function deleteUserMedia(id) {
  assetCache.delete(id);
  imageCache.delete(id);
  const db = await getDb();
  if (db) {
    try {
      const tx = db.transaction(STORE_NAME, 'readwrite');
      tx.objectStore(STORE_NAME).delete(id);
    } catch { /* ignore */ }
  }
}

export function registerAsset(id, dataUrl, name = 'Image') {
  if (!assetCache.has(id)) {
    assetCache.set(id, { id, name, category: 'user', dataUrl, thumb: dataUrl });
  }
}

export function getMediaAsset(id) {
  return assetCache.get(id) || null;
}

export function getAllMedia() {
  const builtin = BUILTINS;
  const user = [];
  for (const [id, a] of assetCache) {
    if (a.category === 'user') user.push(a);
  }
  return { builtin, user };
}

/**
 * Returns an HTMLImageElement for an asset ID or URL.
 * Always returns immediately; `ready` indicates whether the image has decoded and can be drawn.
 */
export function getImage(idOrUrl) {
  if (!idOrUrl) idOrUrl = BUILTINS[0].id;
  let entry = imageCache.get(idOrUrl);
  if (entry) return entry;

  const asset = assetCache.get(idOrUrl);
  const src = asset ? asset.dataUrl : idOrUrl;
  const img = new Image();
  entry = { img, ready: false };
  imageCache.set(idOrUrl, entry);

  img.onload = () => { entry.ready = true; };
  img.onerror = () => {
    // If user asset failed to load, fallback to first builtin
    if (idOrUrl !== BUILTINS[0].id) {
      const fallback = getImage(BUILTINS[0].id);
      entry.img = fallback.img;
      entry.ready = fallback.ready;
    }
  };
  img.src = src;
  return entry;
}
