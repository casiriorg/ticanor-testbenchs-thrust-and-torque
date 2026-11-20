// The desktop build is a plain webview (WebView2/WebKit), so a right-click falls through to
// its native browser context menu ("Save as...", "Inspect", "Reload"...) unless we stop it.
// Nothing in this app currently needs a right-click menu of its own (chart export already has
// its own "Export" button), so this just suppresses the browser one; text fields are left alone
// so native cut/copy/paste still works there.

export function initContextMenu() {
  window.addEventListener("contextmenu", (e) => {
    const target = e.target as HTMLElement | null;
    if (target?.closest('input, textarea, [contenteditable="true"]')) return;
    e.preventDefault();
  });
}
