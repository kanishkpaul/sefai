// <curve-loader type="lemniscate" label="Indexing evidence"></curve-loader>
//
// The shared loading indicator: a faint parametric curve with a comet of particles
// travelling along it, after Hermes desktop's Loader (apps/desktop/src/components/ui/loader.tsx).
// Never ship the literal word "Loading…"; pass a label that says what is happening.
// No dependencies. Works in Svelte, React and plain HTML. Colour follows currentColor.

const TAU = Math.PI * 2

const CURVES = {
  // Long operations.
  lemniscate: { ms: 4200, n: 18, trail: 0.34, stroke: 4.4, point: t => {
    const d = 1 + Math.sin(t) ** 2
    return [50 + (38 * Math.cos(t)) / d, 50 + (38 * Math.sin(t) * Math.cos(t)) / d]
  } },
  // Thinking: a circle with k inward petals.
  thinking: { ms: 5200, n: 16, trail: 0.3, stroke: 5.5, rotate: 36000, point: t => [
    50 + (7 * Math.cos(t) - 3 * Math.cos(7 * t)) * 3.9,
    50 + (7 * Math.sin(t) - 3 * Math.sin(7 * t)) * 3.9
  ] },
  rose: { ms: 4800, n: 16, trail: 0.28, stroke: 4.6, rotate: 28000, point: t => {
    const r = 9.2 * Math.cos(5 * t)
    return [50 + Math.cos(t) * r * 3.25, 50 + Math.sin(t) * r * 3.25]
  } },
  lissajous: { ms: 5600, n: 18, trail: 0.3, stroke: 4.2, point: t => [
    50 + Math.sin(3 * t + 1.57) * 38,
    50 + Math.sin(4 * t) * 35
  ] },
  cardioid: { ms: 4400, n: 14, trail: 0.32, stroke: 4.8, point: t => {
    const r = 15 * (1 - Math.cos(t))
    return [62 + r * Math.cos(t) * 1.25, 50 + r * Math.sin(t) * 1.25]
  } }
}

const NS = 'http://www.w3.org/2000/svg'
const reduced = () => matchMedia('(prefers-reduced-motion: reduce)').matches

class CurveLoader extends HTMLElement {
  connectedCallback() {
    const curve = CURVES[this.getAttribute('type')] ?? CURVES.lemniscate
    const label = this.getAttribute('label')
    this.setAttribute('role', 'status')
    if (label) this.setAttribute('aria-label', label)
    this.style.display = 'inline-flex'
    this.style.alignItems = 'center'
    this.style.gap = '0.5rem'

    const svg = document.createElementNS(NS, 'svg')
    svg.setAttribute('viewBox', '0 0 100 100')
    svg.setAttribute('aria-hidden', 'true')
    svg.style.width = svg.style.height = this.getAttribute('size') ?? '1.25rem'
    svg.style.overflow = 'visible'

    const g = document.createElementNS(NS, 'g')
    const path = document.createElementNS(NS, 'path')
    const steps = 240
    let d = ''
    for (let i = 0; i <= steps; i++) {
      const [x, y] = curve.point((i / steps) * TAU)
      d += `${i ? 'L' : 'M'}${x.toFixed(2)} ${y.toFixed(2)}`
    }
    path.setAttribute('d', d)
    path.setAttribute('fill', 'none')
    path.setAttribute('stroke', 'currentColor')
    path.setAttribute('stroke-opacity', '0.14')
    path.setAttribute('stroke-width', String(curve.stroke * 0.6))
    path.setAttribute('stroke-linecap', 'round')
    g.append(path)

    const dots = Array.from({ length: curve.n }, () => {
      const c = document.createElementNS(NS, 'circle')
      c.setAttribute('fill', 'currentColor')
      g.append(c)
      return c
    })
    svg.append(g)
    this.replaceChildren(svg)
    if (label) {
      const span = document.createElement('span')
      span.textContent = label
      span.style.color = 'var(--ui-text-3, currentColor)'
      span.style.fontSize = 'var(--text-xs, 0.75rem)'
      this.append(span)
    }

    const paint = now => {
      const head = (now % curve.ms) / curve.ms
      dots.forEach((c, i) => {
        const k = i / (curve.n - 1)
        const [x, y] = curve.point((head - k * curve.trail) * TAU)
        c.setAttribute('cx', x.toFixed(2))
        c.setAttribute('cy', y.toFixed(2))
        c.setAttribute('r', ((curve.stroke / 2) * (1 - k * 0.7)).toFixed(2))
        c.setAttribute('fill-opacity', (1 - k).toFixed(2))
      })
      if (curve.rotate) g.setAttribute('transform', `rotate(${((now % curve.rotate) / curve.rotate) * 360} 50 50)`)
    }

    if (reduced()) { paint(0); return }
    const tick = now => {
      if (!this.isConnected) return
      if (!document.hidden) paint(now)
      this.raf = requestAnimationFrame(tick)
    }
    this.raf = requestAnimationFrame(tick)
  }

  disconnectedCallback() {
    cancelAnimationFrame(this.raf)
  }
}

if (!customElements.get('curve-loader')) customElements.define('curve-loader', CurveLoader)
