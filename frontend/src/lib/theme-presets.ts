// All color values are space-separated RGB triplets for Tailwind CSS variable compatibility
// e.g. "248 246 243" → rgb(248 246 243 / <alpha-value>)

export type BaseTheme = 'paper' | 'cloud' | 'slate';
export type AccentColor = 'ocean' | 'emerald' | 'rose' | 'amber' | 'violet' | 'teal';

export interface ThemeConfig {
  mode: 'light' | 'dark';
  base: BaseTheme;
  accent: AccentColor;
}

export const DEFAULT_THEME: ThemeConfig = {
  mode: 'light',
  base: 'paper',
  accent: 'ocean',
};

type ColorScale = Record<string, string>;

// ── Base themes (gray scales) ──────────────────────────────────────

export const bases: Record<BaseTheme, { label: string; swatch: string; surface: string; gray: ColorScale }> = {
  paper: {
    label: 'Paper',
    swatch: '#f1ede8',   // gray-100 representative
    surface: '250 248 245',  // warm off-white for cards
    gray: {
      '50':  '248 246 243',
      '100': '241 237 232',
      '200': '227 222 215',
      '300': '208 201 192',
      '400': '160 152 144',
      '500': '115 107 99',
      '600': '85 78 72',
      '700': '62 57 53',
      '800': '40 37 35',
      '900': '26 24 22',
      '950': '14 13 12',
    },
  },
  cloud: {
    label: 'Cloud',
    swatch: '#f3f4f6',
    surface: '255 255 255',  // pure white
    gray: {
      '50':  '249 250 251',
      '100': '243 244 246',
      '200': '229 231 235',
      '300': '209 213 219',
      '400': '156 163 175',
      '500': '107 114 128',
      '600': '75 85 99',
      '700': '55 65 81',
      '800': '31 41 55',
      '900': '17 24 39',
      '950': '3 7 18',
    },
  },
  slate: {
    label: 'Slate',
    swatch: '#f1f5f9',
    surface: '248 250 253',  // blue-tinted white
    gray: {
      '50':  '248 250 252',
      '100': '241 245 249',
      '200': '226 232 240',
      '300': '203 213 225',
      '400': '148 163 184',
      '500': '100 116 139',
      '600': '71 85 105',
      '700': '51 65 85',
      '800': '30 41 59',
      '900': '15 23 42',
      '950': '2 6 23',
    },
  },
};

// ── Accent colors (primary scales) ─────────────────────────────────

export const accents: Record<AccentColor, { label: string; swatch: string; primary: ColorScale }> = {
  ocean: {
    label: 'Ocean',
    swatch: '#0284c7',  // primary-600
    primary: {
      '50':  '240 247 255',
      '100': '224 238 254',
      '200': '186 221 253',
      '300': '125 197 248',
      '400': '56 174 232',
      '500': '14 143 203',
      '600': '2 116 168',
      '700': '3 93 136',
      '800': '7 77 111',
      '900': '12 63 92',
      '950': '8 42 63',
    },
  },
  emerald: {
    label: 'Emerald',
    swatch: '#059669',
    primary: {
      '50':  '236 253 245',
      '100': '209 250 229',
      '200': '167 243 208',
      '300': '110 231 183',
      '400': '52 211 153',
      '500': '16 185 129',
      '600': '5 150 105',
      '700': '4 120 87',
      '800': '6 95 70',
      '900': '8 75 58',
      '950': '2 44 34',
    },
  },
  rose: {
    label: 'Rose',
    swatch: '#e11d48',
    primary: {
      '50':  '255 241 242',
      '100': '255 228 230',
      '200': '254 205 211',
      '300': '253 164 175',
      '400': '251 113 133',
      '500': '244 63 94',
      '600': '225 29 72',
      '700': '190 18 60',
      '800': '159 18 57',
      '900': '136 19 55',
      '950': '76 5 25',
    },
  },
  amber: {
    label: 'Amber',
    swatch: '#d97706',
    primary: {
      '50':  '255 251 235',
      '100': '254 243 199',
      '200': '253 230 138',
      '300': '252 211 77',
      '400': '251 191 36',
      '500': '245 158 11',
      '600': '217 119 6',
      '700': '180 83 9',
      '800': '146 64 14',
      '900': '120 53 15',
      '950': '69 26 3',
    },
  },
  violet: {
    label: 'Violet',
    swatch: '#7c3aed',
    primary: {
      '50':  '245 243 255',
      '100': '237 233 254',
      '200': '221 214 254',
      '300': '196 181 253',
      '400': '167 139 250',
      '500': '139 92 246',
      '600': '124 58 237',
      '700': '109 40 217',
      '800': '91 33 182',
      '900': '76 29 149',
      '950': '46 16 101',
    },
  },
  teal: {
    label: 'Teal',
    swatch: '#0d9488',
    primary: {
      '50':  '240 253 250',
      '100': '204 251 241',
      '200': '153 246 228',
      '300': '94 234 212',
      '400': '45 212 191',
      '500': '20 184 166',
      '600': '13 148 136',
      '700': '15 118 110',
      '800': '17 94 89',
      '900': '20 78 74',
      '950': '4 47 46',
    },
  },
};

// ── CSS variable generation ────────────────────────────────────────

export function buildCssVars(config: ThemeConfig): string {
  const base = bases[config.base];
  const accent = accents[config.accent];

  const vars: string[] = [];

  // Gray scale
  for (const [shade, value] of Object.entries(base.gray)) {
    vars.push(`--c-gray-${shade}:${value}`);
  }

  // Surface (card white)
  vars.push(`--c-surface:${base.surface}`);

  // Primary scale
  for (const [shade, value] of Object.entries(accent.primary)) {
    vars.push(`--c-primary-${shade}:${value}`);
  }

  return vars.join(';');
}
