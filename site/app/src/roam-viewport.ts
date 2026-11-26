/** 自由画布的位移直接响应输入；补图可以按帧合并，但不能拖住画布移动。 */
export class RoamViewport {
  private x = 0;
  private y = 0;
  private frame = 0;
  private plane: HTMLElement;

  constructor(root: HTMLElement, private changed: (x: number, y: number) => void) {
    this.plane = root.querySelector('.plane')!;
  }

  position(): [number, number] { return [this.x, this.y]; }

  private place(x: number, y: number) {
    this.x = x; this.y = y;
    this.plane.style.transform = `translate3d(${-x}px, ${-y}px, 0)`;
    this.changed(x, y);
  }

  moveTo(x: number, y: number, behavior: ScrollBehavior = 'instant') {
    this.stop();
    if (behavior === 'instant') { this.place(x, y); return; }
    const x0 = this.x, y0 = this.y, start = performance.now();
    const step = (now: number) => {
      const progress = Math.min(1, (now - start) / 520);
      const ease = 1 - Math.pow(1 - progress, 3);
      this.place(x0 + (x - x0) * ease, y0 + (y - y0) * ease);
      this.frame = progress < 1 ? requestAnimationFrame(step) : 0;
    };
    this.frame = requestAnimationFrame(step);
  }

  stop() { cancelAnimationFrame(this.frame); this.frame = 0; }
}
