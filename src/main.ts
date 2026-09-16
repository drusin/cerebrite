import { invoke } from "@tauri-apps/api/core";

async function ping() {
  const resultEl = document.querySelector<HTMLElement>("#ping-result");
  if (!resultEl) return;
  resultEl.textContent = await invoke("ping");
}

window.addEventListener("DOMContentLoaded", () => {
  document.querySelector("#ping-button")?.addEventListener("click", ping);
});
