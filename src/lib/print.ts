import type { PDFDocumentProxy } from "./pdf";

/**
 * Prints the document through the system print dialog: every page is
 * rendered at print resolution into a hidden container that only shows when
 * printing, then the dialog opens.
 */
export async function printDocument(pdf: PDFDocumentProxy, onProgress?: (done: number, total: number) => void): Promise<void> {
  document.getElementById("print-root")?.remove();
  const root = document.createElement("div");
  root.id = "print-root";
  const urls: string[] = [];
  const dpi = 200;
  for (let i = 1; i <= pdf.numPages; i++) {
    const page = await pdf.getPage(i);
    const base = page.getViewport({ scale: 1 });
    const vp = page.getViewport({ scale: dpi / 72 });
    const canvas = document.createElement("canvas");
    canvas.width = Math.floor(vp.width);
    canvas.height = Math.floor(vp.height);
    const ctx = canvas.getContext("2d");
    if (!ctx) continue;
    ctx.fillStyle = "#ffffff";
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    await page.render({ canvas, viewport: vp, intent: "print" }).promise;
    const blob: Blob | null = await new Promise((r) => canvas.toBlob(r, "image/png"));
    if (!blob) continue;
    const url = URL.createObjectURL(blob);
    urls.push(url);
    const img = document.createElement("img");
    img.src = url;
    // Physical size, so each page prints at its own size.
    img.style.width = `${(base.width / 72).toFixed(3)}in`;
    img.style.height = `${(base.height / 72).toFixed(3)}in`;
    root.appendChild(img);
    onProgress?.(i, pdf.numPages);
  }
  document.body.appendChild(root);
  await Promise.all([...root.querySelectorAll("img")].map((img) => img.decode().catch(() => {})));
  const cleanup = () => {
    root.remove();
    urls.forEach((u) => URL.revokeObjectURL(u));
    window.removeEventListener("afterprint", cleanup);
  };
  window.addEventListener("afterprint", cleanup);
  window.print();
}
