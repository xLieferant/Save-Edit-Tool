// settings.js
// Benutzt Tauri `invoke` (window.__TAURI__.invoke oder import { invoke } from '@tauri-apps/api');


import { invoke } from '@tauri-apps/api';


function normalizeValueForBackend(value) {
// Arrays -> SII-like tuple: (v1, v2, v3)
if (Array.isArray(value)) {
return `(${value.join(', ')})`;
}


// boolean -> 1/0
if (typeof value === 'boolean') {
return value ? '1' : '0';
}


// numbers -> as-is
if (typeof value === 'number') {
return value.toString();
}


// null/undefined -> empty string
if (value === null || value === undefined) return '';


// string -> send raw, backend entscheidet ob quotes nötig
return value.toString();
}


export function applySetting(key, value, fileType = 'save') {
const normalized = normalizeValueForBackend(value);
return invoke('apply_setting', { key, value: normalized, file_type: fileType });
}


// Convenience wrappers für UI
export function applyText(key, text, fileType) {
return applySetting(key, text, fileType);
}


export function applyNumber(key, number, fileType) {
return applySetting(key, Number(number), fileType);
}


export function applySlider(key, number, fileType) {
return applyNumber(key, number, fileType);
}


export function applyMulti(key, arrayValues, fileType) {
return applySetting(key, arrayValues, fileType);
}


export function applyCheckbox(key, checked, fileType) {
return applySetting(key, checked ? 1 : 0, fileType);
}