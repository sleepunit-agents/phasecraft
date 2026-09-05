// Checks are independent of the 40 ms musical display loop.
export function setupUpdates(invoke, chip, version, action) {
  let available = null,
    checking = false,
    installing = false;
  async function check() {
    if (checking || installing) return;
    checking = true;
    try {
      const status = await invoke("check_update");
      available = status.commit;
      chip.hidden = !available;
      chip.textContent = available
        ? `↥ Update & restart · ${available.slice(0, 7)}`
        : "";
      chip.title =
        "Stops playback, installs the signed update, and restarts Phasecraft";
      version.title = status.supported
        ? "Click to check for updates · checked just now"
        : "Update this development build or Linux .deb manually. Linux AppImage supports in-app updates.";
    } catch {
      version.title = "Could not check for updates. Click to retry.";
      chip.hidden = false;
      chip.textContent = "↻ Retry update check";
      chip.title =
        "The release may be publishing, or you may be offline. Playback is unaffected.";
      available = null;
    } finally {
      checking = false;
    }
  }
  chip.onclick = () => {
    if (!available) return check();
    return action(async () => {
      installing = true;
      chip.disabled = true;
      chip.textContent = "Installing update…";
      try {
        await invoke("install_update");
      } finally {
        installing = false;
        chip.disabled = false;
        available = null;
        chip.textContent = "↻ Retry update check";
      }
    });
  };
  version.setAttribute("role", "button");
  version.tabIndex = 0;
  version.onclick = check;
  version.onkeydown = (e) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      e.stopPropagation();
      check();
    }
  };
  check();
  setInterval(check, 5 * 60 * 1000);
}
