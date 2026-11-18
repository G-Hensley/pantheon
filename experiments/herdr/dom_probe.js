// Fixed synthetic DOM input for a local fixture. This is not an OS-keyboard test.
async function runHerdrProbe(mode) {
  const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
  for (let n = 0; n < 150; n++) {
    if (document.querySelector('[role="status"]')?.textContent?.startsWith('Connected')) break;
    await delay(100);
  }
  if (mode === 'codex') {
    [...document.querySelectorAll('button')]
      .find(button => button.textContent === 'Codex (empty profile)')?.click();
    return;
  }
  const textarea = document.querySelector('.xterm-helper-textarea');
  if (!textarea) throw new Error('Fixture terminal did not mount');
  textarea.focus();
  // xterm's paste event follows its normal paste/input path, including bracketed paste.
  const text = mode === 'shell-start'
    ? "export GUI_VALUE=original; printf '\\033[32mGUI \\u03bb\\u754c\\U0001f642\\033[0m\\n'; sleep 3; printf 'OUTPUT_WHILE_CLOSED\\n'"
    : "printf 'PERSISTED=%s\\n' \"$GUI_VALUE\"";
  const data = new DataTransfer();
  data.setData('text/plain', text);
  textarea.dispatchEvent(new ClipboardEvent('paste', { bubbles: true, clipboardData: data }));
  await delay(100);
  textarea.dispatchEvent(new KeyboardEvent('keydown', {
    bubbles: true, key: 'Enter', code: 'Enter', keyCode: 13, which: 13,
  }));
}
