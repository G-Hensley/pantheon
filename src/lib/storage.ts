const CURRENT_PREFIX = "pantheon.";
const LEGACY_PREFIX = "mosaic.";

/** Read Pantheon UI state, adopting the pre-rename key on first access. */
export function readStored(name: string): string | null {
  const currentKey = `${CURRENT_PREFIX}${name}`;
  const current = localStorage.getItem(currentKey);
  if (current !== null) return current;

  const legacyKey = `${LEGACY_PREFIX}${name}`;
  const legacy = localStorage.getItem(legacyKey);
  if (legacy === null) return null;

  try {
    localStorage.setItem(currentKey, legacy);
    localStorage.removeItem(legacyKey);
  } catch {
    // Returning the value still restores this launch. The legacy key remains
    // available for another migration attempt when storage becomes writable.
  }
  return legacy;
}

export function writeStored(name: string, value: string): void {
  localStorage.setItem(`${CURRENT_PREFIX}${name}`, value);
}
