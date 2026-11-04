const PRINT_TIMEOUT_MS = 10 * 60_000; // a dialog nobody answers must not leave the sheet in the page forever

/**
 * Prints one picture through the webview's own print dialog. The page's stylesheet hides everything but
 * `#print-sheet` while printing, so the picture is scaled to the paper whatever format it came from.
 */
export async function printPicture(src: string): Promise<void> {
  const sheet = new Image();
  sheet.id = "print-sheet";
  sheet.alt = "";
  sheet.src = src;
  await sheet.decode();
  document.body.append(sheet);
  try {
    await new Promise<void>((done) => {
      const finish = () => {
        removeEventListener("afterprint", finish);
        clearTimeout(timer);
        done();
      };
      const timer = setTimeout(finish, PRINT_TIMEOUT_MS);
      addEventListener("afterprint", finish);
      print();
    });
  } finally {
    sheet.remove();
  }
}
