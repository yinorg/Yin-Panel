// Wallpapers for the directory theme.
//
// The reference site ships each animated background as its own full-page
// effect file. These are our own implementations of the same effect families:
// one canvas behind the panel, a deterministic single frame for the picker
// thumbnails, and nothing copied from anyone else's assets.

const yinSeedRandom = seed => {
  let value = seed || 1
  return () => {
    value = (value * 1103515245 + 12345) & 0x7fffffff
    return value / 0x7fffffff
  }
}

const yinText = (ctx, text, x, y, size) => {
  ctx.font = `${size}px ui-monospace, SFMono-Regular, Menlo, monospace`
  ctx.fillText(text, x, y)
}

/* ---- 溢彩：流动的彩虹色带 ------------------------------------------ */
const drawRainbow = (ctx, w, h, t) => {
  const rows = 72
  for (let i = 0; i < rows; i++) {
    const shift = Math.sin(t * 0.7 + i * 0.22) * w * 0.3
    const hue = (t * 36 + i * 5) % 360
    const gradient = ctx.createLinearGradient(shift, 0, shift + w, 0)
    gradient.addColorStop(0, `hsl(${hue} 82% 56%)`)
    gradient.addColorStop(0.5, `hsl(${(hue + 70) % 360} 88% 60%)`)
    gradient.addColorStop(1, `hsl(${(hue + 150) % 360} 80% 50%)`)
    ctx.fillStyle = gradient
    ctx.fillRect(0, (i / rows) * h, w, h / rows + 1)
  }
}

/* ---- Rainy：雨夜 --------------------------------------------------- */
const initRain = (w, h) => ({ drops: Array.from({ length: 240 }, () => ({ x: Math.random() * w, y: Math.random() * h, len: 10 + Math.random() * 26, v: 420 + Math.random() * 520 })) })
const drawRain = (ctx, w, h, t, state) => {
  const gradient = ctx.createLinearGradient(0, 0, 0, h)
  gradient.addColorStop(0, '#0b1220')
  gradient.addColorStop(1, '#16283d')
  ctx.fillStyle = gradient
  ctx.fillRect(0, 0, w, h)
  ctx.strokeStyle = 'rgba(170, 214, 255, .55)'
  ctx.lineWidth = 1.2
  for (const drop of state.drops) {
    drop.y += drop.v / 60
    if (drop.y > h + drop.len) {
      drop.y = -drop.len
      drop.x = Math.random() * w
    }
    ctx.beginPath()
    ctx.moveTo(drop.x, drop.y)
    ctx.lineTo(drop.x - 2.5, drop.y + drop.len)
    ctx.stroke()
  }
}

/* ---- 三维线条：旋转的线框 ------------------------------------------ */
const drawWireframe = (ctx, w, h, t) => {
  ctx.fillStyle = '#070a10'
  ctx.fillRect(0, 0, w, h)
  const cx = w / 2
  const cy = h / 2
  const scale = Math.min(w, h) * 0.2
  const rotY = t * 0.4
  const rotX = Math.sin(t * 0.25) * 0.55
  const project = (x, y, z) => {
    const X = x * Math.cos(rotY) - z * Math.sin(rotY)
    let Z = x * Math.sin(rotY) + z * Math.cos(rotY)
    const Y = y * Math.cos(rotX) - Z * Math.sin(rotX)
    Z = y * Math.sin(rotX) + Z * Math.cos(rotX)
    const depth = 7 / (7 + Z)
    return [cx + X * scale * depth, cy + Y * scale * depth, depth]
  }
  const segment = (a, b) => {
    ctx.strokeStyle = `rgba(${90 + a[2] * 120}, ${180 + b[2] * 70}, 255, ${Math.min(1, (a[2] + b[2]) * 0.55)})`
    ctx.beginPath()
    ctx.moveTo(a[0], a[1])
    ctx.lineTo(b[0], b[1])
    ctx.stroke()
  }
  ctx.lineWidth = 1
  for (let i = 0; i < 5; i++) {
    for (let j = 0; j < 5; j++) {
      for (let k = 0; k < 3; k++) {
        const p = project(i - 2, j - 2, k - 1)
        if (i < 4) segment(p, project(i - 1, j - 2, k - 1))
        if (j < 4) segment(p, project(i - 2, j - 1, k - 1))
        if (k < 2) segment(p, project(i - 2, j - 2, k))
      }
    }
  }
}

/* ---- 飘零：樱花 (the reference draws ~1600 pale blossoms over a bright sky) ---- */
const initPetals = (w, h) => ({
  items: Array.from({ length: 240 }, (_, index) => ({
    x: Math.random() * w,
    y: Math.random() * h,
    r: 4 + Math.random() * 9,
    v: 24 + Math.random() * 64,
    a: Math.random() * 6.28,
    spin: (Math.random() - 0.5) * 2.6,
    sway: 0.7 + Math.random() * 1.6,
    tint: index % 3,
  })),
})
const YIN_SAKURA_TINTS = ['rgba(255, 205, 216, .95)', 'rgba(255, 226, 233, .95)', 'rgba(255, 186, 202, .92)']
const drawPetals = (ctx, w, h, t, state) => {
  const sky = ctx.createLinearGradient(0, 0, 0, h)
  sky.addColorStop(0, '#eef7ff')
  sky.addColorStop(0.5, '#fdf2f5')
  sky.addColorStop(1, '#ffe7ee')
  ctx.fillStyle = sky
  ctx.fillRect(0, 0, w, h)
  const glow = ctx.createRadialGradient(w * 0.72, h * 0.22, 0, w * 0.72, h * 0.22, Math.max(w, h) * 0.65)
  glow.addColorStop(0, 'rgba(255, 255, 255, .8)')
  glow.addColorStop(1, 'rgba(255, 255, 255, 0)')
  ctx.fillStyle = glow
  ctx.fillRect(0, 0, w, h)
  for (const petal of state.items) {
    petal.y += petal.v / 60
    petal.a += petal.spin / 60
    petal.x += Math.sin(t * petal.sway + petal.a) * 0.9
    if (petal.y > h + 26) {
      petal.y = -26
      petal.x = Math.random() * w
    }
    ctx.save()
    ctx.translate(petal.x, petal.y)
    ctx.rotate(petal.a)
    ctx.fillStyle = YIN_SAKURA_TINTS[petal.tint]
    ctx.beginPath()
    ctx.moveTo(0, 0)
    ctx.bezierCurveTo(petal.r * 0.72, -petal.r * 0.5, petal.r * 0.62, -petal.r * 1.6, 0, -petal.r * 1.95)
    ctx.bezierCurveTo(-petal.r * 0.62, -petal.r * 1.6, -petal.r * 0.72, -petal.r * 0.5, 0, 0)
    ctx.closePath()
    ctx.fill()
    ctx.restore()
  }
}

/* ---- 经典气泡 ------------------------------------------------------ */
const initBubbles = (w, h) => ({ items: Array.from({ length: 60 }, () => ({ x: Math.random() * w, y: h + Math.random() * h, r: 6 + Math.random() * 26, v: 30 + Math.random() * 70, a: Math.random() * 6.28 })) })
const drawBubbles = (ctx, w, h, t, state) => {
  const gradient = ctx.createLinearGradient(0, 0, 0, h)
  gradient.addColorStop(0, '#06222e')
  gradient.addColorStop(1, '#0d4a5c')
  ctx.fillStyle = gradient
  ctx.fillRect(0, 0, w, h)
  for (const item of state.items) {
    item.y -= item.v / 60
    item.x += Math.sin(t * 1.4 + item.a) * 0.6
    if (item.y < -item.r) {
      item.y = h + item.r
      item.x = Math.random() * w
    }
    ctx.strokeStyle = 'rgba(190, 245, 255, .55)'
    ctx.fillStyle = 'rgba(190, 245, 255, .10)'
    ctx.lineWidth = 1.2
    ctx.beginPath()
    ctx.arc(item.x, item.y, item.r, 0, Math.PI * 2)
    ctx.fill()
    ctx.stroke()
    ctx.beginPath()
    ctx.arc(item.x - item.r * 0.3, item.y - item.r * 0.35, item.r * 0.18, 0, Math.PI * 2)
    ctx.fillStyle = 'rgba(255, 255, 255, .5)'
    ctx.fill()
  }
}

/* ---- butterflys：蝶 ------------------------------------------------ */
const initButterflies = (w, h) => ({ items: Array.from({ length: 16 }, (_, index) => ({ x: Math.random() * w, y: Math.random() * h, r: 14 + Math.random() * 16, a: Math.random() * 6.28, hue: (index * 47) % 360, vx: (Math.random() - 0.5) * 60, vy: (Math.random() - 0.5) * 40 })) })
const drawButterflies = (ctx, w, h, t, state) => {
  const gradient = ctx.createLinearGradient(0, 0, w, h)
  gradient.addColorStop(0, '#1b1030')
  gradient.addColorStop(1, '#2f1b3d')
  ctx.fillStyle = gradient
  ctx.fillRect(0, 0, w, h)
  for (const item of state.items) {
    item.x += (item.vx / 60) + Math.sin(t * 1.1 + item.a) * 0.8
    item.y += (item.vy / 60) + Math.cos(t * 0.9 + item.a) * 0.6
    if (item.x < -40) item.x = w + 40
    if (item.x > w + 40) item.x = -40
    if (item.y < -40) item.y = h + 40
    if (item.y > h + 40) item.y = -40
    const flap = Math.abs(Math.sin(t * 9 + item.a))
    ctx.save()
    ctx.translate(item.x, item.y)
    ctx.rotate(Math.sin(t + item.a) * 0.25)
    ctx.fillStyle = `hsla(${item.hue}, 85%, 65%, .85)`
    ctx.beginPath()
    ctx.ellipse(-item.r * 0.55 * flap, 0, item.r * 0.6 * flap, item.r * 0.42, 0, 0, Math.PI * 2)
    ctx.fill()
    ctx.beginPath()
    ctx.ellipse(item.r * 0.55 * flap, 0, item.r * 0.6 * flap, item.r * 0.42, 0, 0, Math.PI * 2)
    ctx.fill()
    ctx.fillStyle = 'rgba(30, 12, 40, .9)'
    ctx.fillRect(-1.2, -item.r * 0.5, 2.4, item.r)
    ctx.restore()
  }
}

/* ---- 数字雨 -------------------------------------------------------- */
const initMatrix = (w, h) => {
  const size = 16
  const columns = Math.ceil(w / size)
  return { size, columns, drops: Array.from({ length: columns }, () => Math.random() * h), random: yinSeedRandom(7) }
}
const drawMatrix = (ctx, w, h, t, state) => {
  ctx.fillStyle = 'rgba(3, 10, 6, .18)'
  ctx.fillRect(0, 0, w, h)
  for (let i = 0; i < state.columns; i++) {
    const y = state.drops[i]
    ctx.fillStyle = 'rgba(180, 255, 200, .95)'
    yinText(ctx, String.fromCharCode(0x30a0 + Math.floor(state.random() * 90)), i * state.size, y, 15)
    ctx.fillStyle = 'rgba(0, 200, 110, .55)'
    for (let k = 1; k < 12; k++) yinText(ctx, String.fromCharCode(0x30a0 + Math.floor(state.random() * 90)), i * state.size, y - k * state.size, 15)
    state.drops[i] = y > h + Math.random() * 900 ? 0 : y + state.size * 0.7
  }
}

/* ---- 色彩渐变 ------------------------------------------------------ */
const drawGradient = (ctx, w, h, t) => {
  const shift = t * 0.08
  const gradient = ctx.createLinearGradient(w * (0.1 + Math.sin(shift) * 0.1), 0, w * (0.9 + Math.cos(shift) * 0.1), h)
  gradient.addColorStop(0, `hsl(${(shift * 90) % 360} 78% 52%)`)
  gradient.addColorStop(0.45, `hsl(${(shift * 90 + 120) % 360} 74% 46%)`)
  gradient.addColorStop(1, `hsl(${(shift * 90 + 240) % 360} 80% 40%)`)
  ctx.fillStyle = gradient
  ctx.fillRect(0, 0, w, h)
  ctx.globalAlpha = 0.22
  ctx.fillStyle = `hsl(${(shift * 120 + 60) % 360} 90% 65%)`
  ctx.beginPath()
  ctx.arc(w * (0.5 + Math.sin(t * 0.3) * 0.3), h * 0.4, Math.min(w, h) * 0.45, 0, Math.PI * 2)
  ctx.fill()
  ctx.globalAlpha = 1
}

/* ---- Starry Night -------------------------------------------------- */
const initStars = (w, h) => {
  const random = yinSeedRandom(11)
  return { stars: Array.from({ length: 260 }, () => ({ x: random() * w, y: random() * h * 0.85, r: 0.4 + random() * 1.5, p: random() * 6.28 })) }
}
const drawStars = (ctx, w, h, t, state) => {
  const gradient = ctx.createLinearGradient(0, 0, 0, h)
  gradient.addColorStop(0, '#050a1f')
  gradient.addColorStop(0.6, '#0b1740')
  gradient.addColorStop(1, '#16224f')
  ctx.fillStyle = gradient
  ctx.fillRect(0, 0, w, h)
  for (const star of state.stars) {
    const twinkle = 0.45 + Math.abs(Math.sin(t * 1.4 + star.p)) * 0.55
    ctx.fillStyle = `rgba(255, 255, 255, ${twinkle})`
    ctx.beginPath()
    ctx.arc(star.x, star.y, star.r, 0, Math.PI * 2)
    ctx.fill()
  }
  ctx.globalAlpha = 0.20
  for (const [cx, cy, radius, color] of [[w * 0.3, h * 0.35, 220, '120, 90, 255'], [w * 0.72, h * 0.5, 180, '40, 140, 255']]) {
    const nebula = ctx.createRadialGradient(cx, cy, 0, cx, cy, radius)
    nebula.addColorStop(0, `rgba(${color}, .9)`)
    nebula.addColorStop(1, 'rgba(0, 0, 0, 0)')
    ctx.fillStyle = nebula
    ctx.fillRect(cx - radius, cy - radius, radius * 2, radius * 2)
  }
  ctx.globalAlpha = 1
  ctx.fillStyle = 'rgba(220, 235, 255, .92)'
  ctx.beginPath()
  ctx.arc(w * 0.78, h * 0.22, 26, 0, Math.PI * 2)
  ctx.fill()
}

/* ---- 蔓延：生长的枝条 ---------------------------------------------- */
const initSpread = () => {
  const random = yinSeedRandom(23)
  const branches = []
  const grow = (x, y, angle, width, depth) => {
    const length = 26 + random() * 34
    const nx = x + Math.cos(angle) * length
    const ny = y + Math.sin(angle) * length
    branches.push({ x1: x, y1: y, x2: nx, y2: ny, width, delay: branches.length * 0.035, depth })
    if (depth < 7) {
      grow(nx, ny, angle - 0.32 - random() * 0.35, width * 0.72, depth + 1)
      grow(nx, ny, angle + 0.32 + random() * 0.35, width * 0.72, depth + 1)
    }
  }
  grow(0, 0, -Math.PI / 2, 6, 0)
  return { branches }
}
const drawSpread = (ctx, w, h, t, state) => {
  ctx.fillStyle = '#05100d'
  ctx.fillRect(0, 0, w, h)
  const cycle = 9
  const progress = (t % cycle) / cycle
  ctx.save()
  ctx.translate(w / 2, h + 40)
  ctx.lineCap = 'round'
  for (const branch of state.branches) {
    const reveal = Math.max(0, Math.min(1, (progress - branch.delay) * 6))
    if (!reveal) continue
    ctx.strokeStyle = `hsla(${140 + branch.depth * 12 - progress * 40}, 80%, ${45 + branch.depth * 3}%, ${0.85 * (1 - progress * 0.35)})`
    ctx.lineWidth = branch.width
    ctx.beginPath()
    ctx.moveTo(branch.x1, branch.y1)
    ctx.lineTo(branch.x1 + (branch.x2 - branch.x1) * reveal, branch.y1 + (branch.y2 - branch.y1) * reveal)
    ctx.stroke()
  }
  ctx.restore()
}

/* ---- 雪夜 ---------------------------------------------------------- */
const initSnow = (w, h) => ({ flakes: Array.from({ length: 220 }, () => ({ x: Math.random() * w, y: Math.random() * h, r: 0.8 + Math.random() * 2.6, v: 18 + Math.random() * 46, a: Math.random() * 6.28 })) })
const drawSnow = (ctx, w, h, t, state) => {
  const gradient = ctx.createLinearGradient(0, 0, 0, h)
  gradient.addColorStop(0, '#0a1430')
  gradient.addColorStop(1, '#1d3560')
  ctx.fillStyle = gradient
  ctx.fillRect(0, 0, w, h)
  ctx.fillStyle = 'rgba(150, 205, 255, .10)'
  ctx.beginPath()
  ctx.arc(w * 0.8, h * 0.2, 70, 0, Math.PI * 2)
  ctx.fill()
  for (const flake of state.flakes) {
    flake.y += flake.v / 60
    flake.x += Math.sin(t * 0.8 + flake.a) * 0.55
    if (flake.y > h + 4) {
      flake.y = -4
      flake.x = Math.random() * w
    }
    ctx.fillStyle = `rgba(255, 255, 255, ${0.4 + flake.r * 0.2})`
    ctx.beginPath()
    ctx.arc(flake.x, flake.y, flake.r, 0, Math.PI * 2)
    ctx.fill()
  }
}

/* ---- 粉色系 -------------------------------------------------------- */
const initPink = (w, h) => ({ hearts: Array.from({ length: 34 }, () => ({ x: Math.random() * w, y: h + Math.random() * h, r: 6 + Math.random() * 14, v: 12 + Math.random() * 26, a: Math.random() * 6.28 })) })
const drawPink = (ctx, w, h, t, state) => {
  const gradient = ctx.createLinearGradient(0, 0, w, h)
  gradient.addColorStop(0, '#ffe3ef')
  gradient.addColorStop(0.5, '#ffc7de')
  gradient.addColorStop(1, '#f8a8c9')
  ctx.fillStyle = gradient
  ctx.fillRect(0, 0, w, h)
  for (const heart of state.hearts) {
    heart.y -= heart.v / 60
    heart.x += Math.sin(t * 1.2 + heart.a) * 0.5
    if (heart.y < -heart.r) {
      heart.y = h + heart.r
      heart.x = Math.random() * w
    }
    const size = heart.r
    ctx.save()
    ctx.translate(heart.x, heart.y)
    ctx.rotate(Math.sin(t + heart.a) * 0.2)
    ctx.fillStyle = `rgba(255, 255, 255, ${0.35 + Math.sin(t * 2 + heart.a) * 0.12})`
    ctx.beginPath()
    ctx.moveTo(0, size * 0.3)
    ctx.bezierCurveTo(size, -size * 0.5, size * 0.5, -size * 1.3, 0, -size * 0.6)
    ctx.bezierCurveTo(-size * 0.5, -size * 1.3, -size, -size * 0.5, 0, size * 0.3)
    ctx.fill()
    ctx.restore()
  }
}

/* ---- Wave ---------------------------------------------------------- */
const drawWave = (ctx, w, h, t) => {
  const gradient = ctx.createLinearGradient(0, 0, 0, h)
  gradient.addColorStop(0, '#02121f')
  gradient.addColorStop(1, '#062b46')
  ctx.fillStyle = gradient
  ctx.fillRect(0, 0, w, h)
  for (let layer = 0; layer < 6; layer++) {
    const base = h * (0.42 + layer * 0.09)
    const amp = 24 + layer * 10
    const speed = 0.5 + layer * 0.16
    ctx.beginPath()
    ctx.moveTo(0, h)
    for (let x = 0; x <= w; x += 12)
      ctx.lineTo(x, base + Math.sin(x * 0.008 + t * speed) * amp + Math.sin(x * 0.021 - t * speed * 1.4) * amp * 0.4)
    ctx.lineTo(w, h)
    ctx.closePath()
    ctx.fillStyle = `hsla(${188 + layer * 9}, ${68 + layer * 4}%, ${32 + layer * 5}%, .78)`
    ctx.fill()
  }
}

/* ---- 烟火 ---------------------------------------------------------- */
const drawFireworks = (ctx, w, h, t, state) => {
  ctx.fillStyle = 'rgba(4, 6, 14, .28)'
  ctx.fillRect(0, 0, w, h)
  if (t > state.next || !state.sparks.length) {
    state.next = t + 0.45 + Math.random() * 0.6
    const cx = w * (0.2 + Math.random() * 0.6)
    const cy = h * (0.15 + Math.random() * 0.4)
    const hue = Math.random() * 360
    for (let i = 0; i < 90; i++) {
      const angle = (i / 90) * Math.PI * 2
      const speed = 40 + Math.random() * 130
      state.sparks.push({ x: cx, y: cy, vx: Math.cos(angle) * speed, vy: Math.sin(angle) * speed, life: 1, hue: hue + Math.random() * 40 })
    }
  }
  ctx.globalCompositeOperation = 'lighter'
  for (const spark of state.sparks) {
    spark.x += spark.vx / 60
    spark.y += spark.vy / 60
    spark.vy += 34 / 60
    spark.vx *= 0.99
    spark.vy *= 0.99
    spark.life -= 0.008
    if (spark.life <= 0) continue
    ctx.fillStyle = `hsla(${spark.hue}, 95%, 62%, ${spark.life})`
    ctx.beginPath()
    ctx.arc(spark.x, spark.y, 1.8, 0, Math.PI * 2)
    ctx.fill()
  }
  ctx.globalCompositeOperation = 'source-over'
  state.sparks = state.sparks.filter(spark => spark.life > 0)
}

/* ---- 流光 ---------------------------------------------------------- */
const initStreamers = (w, h) => ({ streaks: Array.from({ length: 70 }, () => ({ x: Math.random() * w, y: Math.random() * h, len: 60 + Math.random() * 220, v: 180 + Math.random() * 420, hue: Math.random() * 90 + 180 })) })
const drawStreamers = (ctx, w, h, t, state) => {
  ctx.fillStyle = 'rgba(4, 8, 16, .30)'
  ctx.fillRect(0, 0, w, h)
  ctx.globalCompositeOperation = 'lighter'
  ctx.lineCap = 'round'
  for (const streak of state.streaks) {
    streak.y += streak.v / 60
    streak.x += streak.v / 260
    if (streak.y - streak.len > h) {
      streak.y = -streak.len
      streak.x = Math.random() * w
    }
    const gradient = ctx.createLinearGradient(streak.x, streak.y - streak.len, streak.x, streak.y)
    gradient.addColorStop(0, `hsla(${streak.hue}, 90%, 60%, 0)`)
    gradient.addColorStop(1, `hsla(${streak.hue}, 95%, 68%, .85)`)
    ctx.strokeStyle = gradient
    ctx.lineWidth = 1.6
    ctx.beginPath()
    ctx.moveTo(streak.x, streak.y - streak.len)
    ctx.lineTo(streak.x, streak.y)
    ctx.stroke()
  }
  ctx.globalCompositeOperation = 'source-over'
}

const YIN_WALLPAPER_LIST = [
  { id: 'rainbow', name: '溢彩', init: () => ({}), draw: drawRainbow },
  { id: 'rainy', name: 'Rainy', init: initRain, draw: drawRain },
  { id: 'wireframe', name: '三维线条', init: () => ({}), draw: drawWireframe },
  { id: 'petals', name: '飘零', init: initPetals, draw: drawPetals },
  { id: 'bubbles', name: '经典气泡', init: initBubbles, draw: drawBubbles },
  { id: 'butterflys', name: 'butterflys', init: initButterflies, draw: drawButterflies },
  { id: 'matrix', name: '数字雨', init: initMatrix, draw: drawMatrix },
  { id: 'gradient', name: '色彩渐变', init: () => ({}), draw: drawGradient },
  { id: 'starry', name: 'Starry Night', init: initStars, draw: drawStars },
  { id: 'spread', name: '蔓延', init: initSpread, draw: drawSpread },
  { id: 'snow', name: '雪夜', init: initSnow, draw: drawSnow },
  { id: 'pink', name: '粉色系', init: initPink, draw: drawPink },
  { id: 'wave', name: 'Wave', init: () => ({}), draw: drawWave },
  { id: 'fireworks', name: '烟火', init: () => ({ sparks: [], next: 0 }), draw: drawFireworks },
  { id: 'streamer', name: '流光', init: initStreamers, draw: drawStreamers },
]

const YIN_WALLPAPER_KEY = 'yin-theme-wallpaper'

const readYinWallpaper = () => {
  try { return localStorage.getItem(YIN_WALLPAPER_KEY) || '' } catch { return '' }
}
const writeYinWallpaper = id => {
  try {
    if (id) localStorage.setItem(YIN_WALLPAPER_KEY, id)
    else localStorage.removeItem(YIN_WALLPAPER_KEY)
  }
  catch { /* private mode */ }
}

/**
 * Paints the selected wallpaper on a full-viewport canvas behind the panel.
 * `prefers-reduced-motion` keeps the first frame and stops.
 */
const createYinWallpaperLayer = (host, resolveAsset) => {
  const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches
  const canvas = element('canvas', 'yin-wallpaper')
  canvas.setAttribute('aria-hidden', 'true')
  const context = canvas.getContext('2d')
  host.prepend(canvas)
  let effect = null
  let picture = null
  let state = null
  let frame = 0
  let startedAt = 0
  let visible = document.visibilityState !== 'hidden'
  const dpr = Math.min(window.devicePixelRatio || 1, 2)

  const size = () => {
    const w = window.innerWidth
    const h = window.innerHeight
    canvas.width = Math.round(w * dpr)
    canvas.height = Math.round(h * dpr)
    context.setTransform(dpr, 0, 0, dpr, 0, 0)
    return [w, h]
  }
  const paintPicture = () => {
    if (!picture || !picture.complete || !picture.naturalWidth) return
    const [w, h] = size()
    const scale = Math.max(w / picture.naturalWidth, h / picture.naturalHeight)
    const dw = picture.naturalWidth * scale
    const dh = picture.naturalHeight * scale
    context.drawImage(picture, (w - dw) / 2, (h - dh) / 2, dw, dh)
  }
  const resize = () => {
    const [w, h] = size()
    if (picture) {
      paintPicture()
      return
    }
    state = effect ? effect.init(w, h) : null
    if (effect && reduced) effect.draw(context, w, h, 0, state)
  }
  const tick = now => {
    frame = requestAnimationFrame(tick)
    if (!effect || !visible || document.hidden) return
    effect.draw(context, window.innerWidth, window.innerHeight, (now - startedAt) / 1000, state)
  }
  const clear = () => {
    context.setTransform(1, 0, 0, 1, 0, 0)
    context.clearRect(0, 0, canvas.width, canvas.height)
  }
  const apply = value => {
    if (frame) cancelAnimationFrame(frame)
    frame = 0
    effect = null
    picture = null
    state = null
    canvas.hidden = !value
    if (!value) {
      clear()
      return
    }
    size()
    if (value.startsWith('image:')) {
      const source = resolveAsset(value.slice(6))
      if (!source) {
        canvas.hidden = true
        return
      }
      picture = new Image()
      picture.referrerPolicy = 'no-referrer'
      picture.addEventListener('load', paintPicture)
      picture.src = source
      return
    }
    effect = YIN_WALLPAPER_LIST.find(entry => entry.id === value) || null
    canvas.hidden = !effect
    if (!effect) {
      clear()
      return
    }
    state = effect.init(window.innerWidth, window.innerHeight)
    startedAt = performance.now()
    if (reduced) {
      effect.draw(context, window.innerWidth, window.innerHeight, 0, state)
      return
    }
    frame = requestAnimationFrame(tick)
  }
  window.addEventListener('resize', resize)
  document.addEventListener('visibilitychange', () => {
    visible = document.visibilityState !== 'hidden'
  })
  apply(readYinWallpaper())
  return { canvas, apply, current: readYinWallpaper }
}

const YIN_WALLPAPER_PAGE_SIZE = 24

/**
 * The wallpaper centre: the reference catalogue's categories, one page of tiles
 * at a time, with a live single-frame preview per effect and the packaged image
 * for the photo categories.
 */
const createYinWallpaperCenter = (layer, catalogue, resolveAsset, requestRender) => {
  const backdrop = element('div', 'yin-wallpaper-center')
  backdrop.hidden = true
  const sheet = element('section', 'yin-wallpaper-sheet')
  sheet.setAttribute('role', 'dialog')
  sheet.setAttribute('aria-modal', 'true')
  sheet.setAttribute('aria-label', '壁纸中心')
  const header = element('header', 'yin-wallpaper-header')
  const heading = element('h2', 'yin-wallpaper-title', '壁纸中心')
  const close = element('button', 'yin-wallpaper-close', '×')
  close.type = 'button'
  close.setAttribute('aria-label', '关闭壁纸中心')
  header.append(heading, close)

  const tabs = element('div', 'yin-wallpaper-tabs')
  const grid = element('div', 'yin-wallpaper-grid')
  const pager = element('div', 'yin-wallpaper-pager')
  const previous = element('button', 'yin-wallpaper-page', '‹')
  previous.type = 'button'
  previous.setAttribute('aria-label', '上一页')
  const status = element('span', 'yin-wallpaper-page-status')
  const next = element('button', 'yin-wallpaper-page', '›')
  next.type = 'button'
  next.setAttribute('aria-label', '下一页')
  pager.append(previous, status, next)

  let category = catalogue[0] || { items: [] }
  let page = 0

  const tile = (value, name, kind) => {
    const item = element('button', 'yin-wallpaper-item')
    item.type = 'button'
    item.dataset.wallpaper = value
    const caption = element('h3', 'yin-wallpaper-name', name)
    let preview
    if (kind && kind.effect) {
      // Effects are painted once at the device pixel ratio, so the still reads
      // sharp on a HiDPI screen.
      preview = element('canvas', 'yin-wallpaper-preview')
      const dpr = Math.min(window.devicePixelRatio || 1, 2)
      preview.width = 192 * dpr
      preview.height = 108 * dpr
      const context = preview.getContext('2d')
      context.scale(dpr, dpr)
      kind.effect.draw(context, 192, 108, 2.4, kind.effect.init(192, 108))
    }
    else if (kind && kind.image) {
      // A photo preview is the photo itself: the browser scales it, so it is as
      // sharp as the file and loads lazily as the page scrolls.
      preview = element('img', 'yin-wallpaper-preview')
      preview.loading = 'lazy'
      preview.alt = ''
      preview.src = resolveAsset(kind.image)
    }
    else {
      preview = element('span', 'yin-wallpaper-preview yin-wallpaper-preview-empty')
    }
    item.append(preview, caption)
    item.addEventListener('click', () => {
      layer.apply(value)
      writeYinWallpaper(value)
      requestRender()
      decorate()
    })
    return item
  }

  const decorate = () => {
    const active = readYinWallpaper()
    for (const node of grid.querySelectorAll('.yin-wallpaper-item')) {
      const on = node.dataset.wallpaper === active
      node.classList.toggle('active', on)
      node.setAttribute('aria-pressed', on ? 'true' : 'false')
    }
    for (const node of tabs.querySelectorAll('.yin-wallpaper-tab')) {
      const on = node.dataset.category === category.slug
      node.classList.toggle('active', on)
      node.setAttribute('aria-pressed', on ? 'true' : 'false')
    }
    const pages = Math.max(1, Math.ceil(category.items.length / YIN_WALLPAPER_PAGE_SIZE))
    status.textContent = `${page + 1} / ${pages}`
    previous.disabled = page <= 0
    next.disabled = page >= pages - 1
  }

  const render = () => {
    grid.replaceChildren()
    const start = page * YIN_WALLPAPER_PAGE_SIZE
    const slice = category.items.slice(start, start + YIN_WALLPAPER_PAGE_SIZE)
    if (page === 0) grid.append(tile('', '默认', null))
    for (const entry of slice) {
      if (entry.effect) {
        const effect = YIN_WALLPAPER_LIST.find(candidate => candidate.id === entry.effect)
        grid.append(tile(entry.effect, entry.name, effect ? { effect } : null))
      }
      else if (entry.file) {
        grid.append(tile(`image:${entry.file}`, entry.name, { image: entry.file }))
      }
    }
    decorate()
  }

  const selectCategory = next => {
    category = next
    page = 0
    render()
  }
  for (const entry of catalogue) {
    const tab = element('button', 'yin-wallpaper-tab', entry.name)
    tab.type = 'button'
    tab.dataset.category = entry.slug
    tab.addEventListener('click', () => selectCategory(entry))
    tabs.append(tab)
  }
  previous.addEventListener('click', () => {
    if (page > 0) {
      page -= 1
      render()
    }
  })
  next.addEventListener('click', () => {
    const pages = Math.max(1, Math.ceil(category.items.length / YIN_WALLPAPER_PAGE_SIZE))
    if (page < pages - 1) {
      page += 1
      render()
    }
  })
  close.addEventListener('click', () => { backdrop.hidden = true })
  backdrop.addEventListener('pointerdown', event => {
    if (event.target === backdrop) backdrop.hidden = true
  })
  document.addEventListener('keydown', event => {
    if (event.key === 'Escape' && !backdrop.hidden) backdrop.hidden = true
  })
  sheet.append(header, tabs, grid, pager)
  backdrop.append(sheet)
  render()
  return {
    element: backdrop,
    open() {
      render()
      backdrop.hidden = false
      close.focus()
    },
  }
}
