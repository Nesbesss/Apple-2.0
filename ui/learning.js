// Merges the fixes the user made to dictated text into the vocabulary. Pure functions, so the rules are
// tested outside the app. A learned entry is marked, so the list can show where it came from.
(function (root) {
  const same = (a, b) => a.toLowerCase() === b.toLowerCase();

  // corrections: [{ heard, meant }] from the backend, where heard is null for a capitals-only fix.
  // Returns the new list and the entries that were added or changed, so the UI can say what was learned.
  function mergeLearned(vocabulary, corrections, max) {
    const next = vocabulary.slice();
    const added = [];
    for (const correction of corrections) {
      if (next.length >= max) break;
      const word = (correction.meant || "").trim();
      const heard = (correction.heard || "").trim();
      if (!word) continue;
      // Never swap out a word the user already has, so a fix cannot undo an entry.
      if (heard && next.some((entry) => same(entry.word, heard))) continue;
      const index = next.findIndex((entry) => same(entry.word, word));
      if (index === -1) {
        const entry = { word, heardAs: heard, learned: true };
        next.push(entry);
        added.push(entry);
        continue;
      }
      // A capitals-only fix needs no swap: the word is already known.
      if (!heard) continue;
      const existing = next[index];
      if (!existing.heardAs) {
        const updated = { ...existing, heardAs: heard, learned: true };
        next[index] = updated;
        added.push(updated);
      } else if (!same(existing.heardAs, heard)) {
        // A second misheard spelling of the same word gets its own entry, so the first is kept.
        const entry = { word: existing.word, heardAs: heard, learned: true };
        next.push(entry);
        added.push(entry);
      }
    }
    return { vocabulary: next, added };
  }

  const api = { mergeLearned };
  if (typeof module !== "undefined" && module.exports) module.exports = api;
  root.NesaLearning = api;
})(typeof window !== "undefined" ? window : globalThis);
