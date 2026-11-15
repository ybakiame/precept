// Only the NSH browser renderer: app, state and handlers are compiled Rust.
import { createPlayer } from './nsh.js';
try {
  const player = await createPlayer(document.getElementById('app'), {
    wasm: './acp_operator_ui.wasm', idiom: 'desktop',
    onError(error) { document.getElementById('error').textContent = String(error); },
  });
  player.runRust();
  window.acpOfflinePlayer = player;
} catch (error) { document.getElementById('error').textContent = String(error); }
