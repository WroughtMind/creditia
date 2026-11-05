// Runs inside the original application origin, including with a minimal helper
// page. Keep the old storage untouched so returning to the old app remains safe.
(async () => {
  if (window.__creditiaMigrationStarted) return;
  window.__creditiaMigrationStarted = true;
  const invoke = window.__TAURI__.core.invoke;
  try {
    const values = {};
    const managed = key => ['lang', 'theme', 'view', 'names', 'autofresh', 'fmt', 'size', 'project', 'logo'].includes(key)
      || key.startsWith('group.') || key.startsWith('autofresh.');
    for (let i = 0; i < localStorage.length; i++) {
      const key = localStorage.key(i);
      if (key && key.startsWith('creditia.') && managed(key.slice(9))) values[key] = localStorage.getItem(key);
    }
    await invoke('import_legacy_preferences', { values });
    await invoke('migration_done', { success: true });
  } catch (error) {
    console.error('Creditia preference migration failed', error);
    await invoke('migration_done', { success: false });
  }
})();
