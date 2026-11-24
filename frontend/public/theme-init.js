// Apply the saved theme before the app loads to prevent a flash of the wrong theme.
// Kept as a separate file: the Content-Security-Policy (script-src 'self') blocks inline scripts.
(function () {
  var isDark = false;
  try {
    var raw = localStorage.getItem('dynamight-theme');
    if (raw) {
      var config = JSON.parse(raw);
      isDark = config.mode === 'dark';
    }
  } catch (e) {
    // Legacy: bare string "dark"
    isDark = localStorage.getItem('dynamight-theme') === 'dark';
  }
  if (isDark || (!localStorage.getItem('dynamight-theme') && window.matchMedia('(prefers-color-scheme: dark)').matches)) {
    document.documentElement.classList.add('dark');
  }
  // Apply cached CSS variables (base + accent colors)
  var vars = localStorage.getItem('dynamight-theme-vars');
  if (vars) document.documentElement.setAttribute('style', vars);
})();
