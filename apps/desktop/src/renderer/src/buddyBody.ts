import { BUDDY_RIG, armJoints, bodyY, headDrop, nearness, neckOf } from './buddyRig.js'
import type { ArmJoints, ArmPose, BuddyPose, Vec3 } from './buddyRig.js'

/**
 * CODEX BUDDY, DRAWN BY LOCUST BELOW HIS HEAD (2026-10-05).
 *
 * Colin, of the rig's first body -- his maker's drawing with its arms cut
 * off: *"his arms look broken when hes shoulder pressing, some real freaky
 * stuff ... maybe make him a little bit buffer"*, and *"the bottom half looks
 * a little cooked / trimmed in ms paint-ish"*. A drawing with its arms cut
 * out keeps its sleeves where the arms were and its legs where they stood.
 * So only his head is his maker's (`buddyHead`: his cap, his hair, his ears;
 * his screen covers his face), and all of him below it is drawn here, in his
 * maker's style -- his ink round flat colour, a shaded side, chunky shapes:
 *
 * - his shirt and its sleeves as ONE shape (inked, then filled), so a sleeve
 *   turns with its arm and meets his shirt with no seam;
 * - his arms built of rounded pieces -- a shoulder, a biceps that swells as
 *   the elbow bends, a forearm thick at the elbow and narrowing to the wrist
 *   -- each arm inked whole and then filled, so its elbow has no seam;
 * - the top of his star shorts (Colin, of the rest: *"do you just want to do
 *   torso up so we can make it bigger"*: he is framed from his cap to his belt).
 *
 * His arms are drawn over his shirt, never over his shoulders: each turns
 * inside its sleeve, and the sleeve is laid over it again.
 */

/** A round piece, or a flat-sided one: what a body part is built from. */
type Piece = { readonly kind: 'disc'; readonly x: number; readonly y: number; readonly r: number } | { readonly kind: 'poly'; readonly points: readonly (readonly [number, number])[] }

interface Ring {
  readonly x: number
  readonly y: number
  readonly r: number
}

const disc = (ring: Ring): Piece => ({ kind: 'disc', x: ring.x, y: ring.y, r: ring.r })

/** Clockwise as the picture is drawn (y down), so pieces added to one path all fill together. */
function clockwise(points: readonly (readonly [number, number])[]): readonly (readonly [number, number])[] {
  let area = 0
  for (let i = 0; i < points.length; i += 1) {
    const [x0, y0] = points[i] as readonly [number, number]
    const [x1, y1] = points[(i + 1) % points.length] as readonly [number, number]
    area += x0 * y1 - x1 * y0
  }
  return area >= 0 ? points : [...points].reverse()
}

/** Two rounds joined by the straight sides that touch both: a tapered limb. */
function taper(a: Ring, b: Ring): readonly Piece[] {
  const dx = b.x - a.x
  const dy = b.y - a.y
  const length = Math.hypot(dx, dy)
  if (length <= Math.abs(a.r - b.r) + 1e-6) return [disc(a), disc(b)]
  const ux = dx / length
  const uy = dy / length
  const c = (a.r - b.r) / length
  const s = Math.sqrt(Math.max(0, 1 - c * c))
  const side = (sign: 1 | -1): [number, number] => [ux * c - uy * s * sign, uy * c + ux * s * sign]
  const [p1x, p1y] = side(1)
  const [p2x, p2y] = side(-1)
  return [
    disc(a),
    disc(b),
    { kind: 'poly', points: clockwise([[a.x + p1x * a.r, a.y + p1y * a.r], [b.x + p1x * b.r, b.y + p1y * b.r], [b.x + p2x * b.r, b.y + p2y * b.r], [a.x + p2x * a.r, a.y + p2y * a.r]]) }
  ]
}

/** A limb through `rings`, each joined to the next. */
const limb = (rings: readonly Ring[]): readonly Piece[] => rings.slice(1).flatMap((ring, i) => taper(rings[i] as Ring, ring))

function trace(context: CanvasRenderingContext2D, pieces: readonly Piece[]): void {
  context.beginPath()
  for (const piece of pieces) {
    if (piece.kind === 'disc') {
      context.moveTo(piece.x + piece.r, piece.y)
      context.arc(piece.x, piece.y, piece.r, 0, Math.PI * 2)
    } else {
      piece.points.forEach(([x, y], i) => (i === 0 ? context.moveTo(x, y) : context.lineTo(x, y)))
      context.closePath()
    }
  }
}

/** A part as his maker draws one: his ink round the whole of it, then its colour over the whole, so pieces meet with no seam. */
function inked(context: CanvasRenderingContext2D, pieces: readonly Piece[], colour: string): void {
  trace(context, pieces)
  context.lineJoin = 'round'
  context.lineWidth = BUDDY_RIG.ink * 2
  context.strokeStyle = BUDDY_RIG.inkColour
  context.stroke()
  // Its colour straight over: never ink under it, which a clipped edge would let show through as a grey line.
  context.fillStyle = colour
  context.fill()
}

/** A part as his maker shades one: a band down its outer side (toward `away`, -1 or 1), `band` wide. */
function shaded(context: CanvasRenderingContext2D, pieces: readonly Piece[], away: number, colour: string, shade: string, band = 3.6): void {
  context.save()
  trace(context, pieces)
  context.clip()
  context.fillStyle = shade
  context.fill()
  context.translate(-away * band, 0)
  trace(context, pieces)
  context.fillStyle = colour
  context.fill()
  context.restore()
}

const at = (a: Vec3, b: Vec3, k: number): Vec3 => ({ x: a.x + (b.x - a.x) * k, y: a.y + (b.y - a.y) * k, z: a.z + (b.z - a.z) * k })

/** An arm's rounds: shoulder, biceps (swelling as the elbow bends, toward the forearm), elbow; forearm, wrist. Nearer, larger. */
function armRings(joints: ArmJoints, pose: ArmPose): { readonly upper: readonly Ring[]; readonly fore: readonly Ring[] } {
  const { shoulder: s, elbow: e, hand: h } = joints
  const size = (v: Vec3, r: number): Ring => ({ x: v.x, y: v.y, r: r * nearness(v.z) })
  // Which way the biceps swells: across the upper arm, toward the forearm.
  const ux = e.x - s.x
  const uy = e.y - s.y
  const length = Math.hypot(ux, uy) || 1
  let nx = -uy / length
  let ny = ux / length
  if (nx * (h.x - e.x) + ny * (h.y - e.y) < 0) {
    nx = -nx
    ny = -ny
  }
  const pump = Math.max(0, Math.min(1, pose.bend / 2.2))
  const belly = at(s, e, 0.45)
  return {
    upper: [size(s, 10), { ...size(belly, 10.6 + 2.4 * pump), x: belly.x + nx * 2.2 * pump, y: belly.y + ny * 2.2 * pump }, size(e, 8)],
    fore: [size(e, 8.4), size(at(e, h, 0.32), 9.6), size(at(e, h, 0.86), 6.6)]
  }
}

/** Eased in and out, 0 to 1. */
const smooth = (t: number): number => {
  const k = Math.max(0, Math.min(1, t))
  return k * k * (3 - 2 * k)
}

/**
 * A LIMB WITH A JOINT, as his maker draws one (his curls and presses, rows 7
 * and 8): straight, it is one shape, inked round as one; bent, its nearer part
 * lies over the farther at the joint with an outline of its own -- the
 * forearm over the biceps at the elbow -- so the joint reads as a joint and not a bend in a hose. The overlap's line
 * grows in as the joint bends (`bend`, 0 straight to 1 well bent), never all
 * at once.
 */
function drawJointed(context: CanvasRenderingContext2D, far: readonly Piece[], near: readonly Piece[], side: -1 | 1, bend: number, band = 3.6): void {
  const R = BUDDY_RIG
  const all = [...far, ...near]
  inked(context, all, R.skin)
  shaded(context, all, side, R.skin, R.shade, band)
  const k = smooth((bend - 0.4) / 0.14)
  if (k <= 0) return
  // The near part's own outline, where it lies over the far part...
  context.save()
  trace(context, far)
  context.clip()
  trace(context, near)
  context.lineJoin = 'round'
  // Drawn in as a line growing from nothing, never faded: a faded line reads as a ghost of one.
  context.lineWidth = R.ink * 2 * k
  context.strokeStyle = R.inkColour
  context.stroke()
  context.restore()
  // ...and the near part filled again over it, so only the line outside it shows.
  shaded(context, near, side, R.skin, R.shade, band)
}

/** How far a joint between two segments is bent, 0 straight to PI folded, from its three points. */
function bendAt(a: Vec3, joint: Vec3, b: Vec3): number {
  const ux = a.x - joint.x
  const uy = a.y - joint.y
  const uz = a.z - joint.z
  const vx = b.x - joint.x
  const vy = b.y - joint.y
  const vz = b.z - joint.z
  const cos = (ux * vx + uy * vy + uz * vz) / ((Math.hypot(ux, uy, uz) || 1) * (Math.hypot(vx, vy, vz) || 1))
  return Math.PI - Math.acos(Math.max(-1, Math.min(1, cos)))
}

/** One arm: its upper arm, and its forearm over it at the elbow as the elbow bends; its outer side shaded. */
function drawArm(context: CanvasRenderingContext2D, joints: ArmJoints, pose: ArmPose, side: -1 | 1): void {
  const rings = armRings(joints, pose)
  drawJointed(context, limb(rings.upper), limb(rings.fore), side, bendAt(joints.shoulder, joints.elbow, joints.hand) / 1.6)
}

/** A sleeve's hem: its two corners, across the arm a third of the way down. */
function hemOf(joints: ArmJoints): readonly [readonly [number, number], readonly [number, number]] {
  const s = joints.shoulder
  const hem = at(s, joints.elbow, 0.42)
  const ux = hem.x - s.x
  const uy = hem.y - s.y
  const length = Math.hypot(ux, uy) || 1
  const r = 12 * nearness(hem.z)
  return [[hem.x - (uy / length) * r, hem.y + (ux / length) * r], [hem.x + (uy / length) * r, hem.y - (ux / length) * r]]
}

/** A sleeve on an arm: round at his shoulder, flat at its hem, a third of the way down the upper arm. */
function sleeve(joints: ArmJoints): readonly Piece[] {
  const s = joints.shoulder
  const hem = at(s, joints.elbow, 0.42)
  const near = nearness(hem.z)
  const top = { x: s.x, y: s.y, r: 12.4 }
  const ux = hem.x - s.x
  const uy = hem.y - s.y
  const length = Math.hypot(ux, uy) || 1
  const nx = -uy / length
  const ny = ux / length
  const r = 12 * near
  return [disc(top), { kind: 'poly', points: clockwise([[s.x + nx * top.r, s.y + ny * top.r], [hem.x + nx * r, hem.y + ny * r], [hem.x - nx * r, hem.y - ny * r], [s.x - nx * top.r, s.y - ny * top.r]]) }]
}

/** His shirt's body, his hips dropped `dip`: neck, shoulders, sides, hem. */
function torso(dip: number): Piece {
  const points: readonly (readonly [number, number])[] = [
    [78, 77], [109, 77], [122, 82], [126, 93], [117.5, 104], [119, 131], [68, 131], [69.5, 104], [61, 93], [65, 82]
  ]
  return { kind: 'poly', points: clockwise(points.map(([x, y]) => [x, bodyY(y, dip)] as const)) }
}

/** A five-pointed star. */
function star(context: CanvasRenderingContext2D, x: number, y: number, r: number, turn: number): void {
  context.beginPath()
  for (let i = 0; i < 10; i += 1) {
    const angle = turn - Math.PI / 2 + (i * Math.PI) / 5
    const radius = i % 2 === 0 ? r : r * 0.45
    const px = x + Math.cos(angle) * radius
    const py = y + Math.sin(angle) * radius
    if (i === 0) context.moveTo(px, py)
    else context.lineTo(px, py)
  }
  context.closePath()
  context.fill()
}

/** Where his shorts' stars sit, standing, as his maker drew them: across, down, size, turn. */
const STARS: readonly (readonly [number, number, number, number])[] = [
  [74, 136, 4.6, 0.2], [86, 134, 4.2, -0.1], [101, 135, 4.5, 0.15], [114, 137, 4.3, -0.2],
  [70, 151, 4.4, -0.15], [82, 149, 4.6, 0.1], [97, 151, 4.2, -0.05], [110, 150, 4.6, 0.2], [120, 152, 3.6, 0]
]

/** His shorts, as far as he is ever framed: their top, under his shirt's hem. */
function shorts(dip: number): readonly Piece[] {
  return [{ kind: 'poly', points: clockwise([[64.5, bodyY(128, dip)], [122.5, bodyY(128, dip)], [123, 160 + dip], [64, 160 + dip]]) }]
}

/**
 * THE CODEX MARK ON HIS SHIRT (2026-10-05). Colin: *"you should just give his
 * shirt the codex logo lol ... that way we can keep the nod to codex while
 * having familiarity"*, then *"make the codex logo black and slightly
 * larger"*: the Codex cloud, eight lobes round, and its prompt, `>_`.
 * `radius` is the cloud's, to the outside of its outline.
 */
export function drawCodexMark(context: CanvasRenderingContext2D, x: number, y: number, radius: number): void {
  const R = BUDDY_RIG
  // Bold enough to read at a bot's size.
  const line = radius * 0.23
  const cloud = (grow: number): void => {
    context.beginPath()
    const reach = radius * 0.5
    const lobe = radius * 0.5 - line / 2 + grow
    for (let i = 0; i < 8; i += 1) {
      const angle = (i / 8) * Math.PI * 2 + 0.2
      const cx = x + Math.cos(angle) * reach
      const cy = y + Math.sin(angle) * reach
      context.moveTo(cx + lobe, cy)
      context.arc(cx, cy, lobe, 0, Math.PI * 2)
    }
    context.moveTo(x + reach + grow, y)
    context.arc(x, y, reach + grow, 0, Math.PI * 2)
  }
  context.save()
  cloud(line / 2)
  context.fillStyle = R.markColour
  context.fill('nonzero')
  cloud(-line / 2)
  context.fillStyle = R.shirt
  context.fill('nonzero')
  context.beginPath()
  context.moveTo(x - radius * 0.4, y - radius * 0.27)
  context.lineTo(x - radius * 0.19, y)
  context.lineTo(x - radius * 0.4, y + radius * 0.27)
  context.moveTo(x + radius * 0.08, y + radius * 0.25)
  context.lineTo(x + radius * 0.4, y + radius * 0.25)
  context.lineWidth = line
  context.lineCap = 'round'
  context.lineJoin = 'round'
  context.strokeStyle = R.markColour
  context.stroke()
  context.restore()
}

const armsOf = (pose: BuddyPose): readonly { readonly side: -1 | 1; readonly pose: ArmPose; readonly joints: ArmJoints }[] =>
  ([-1, 1] as const).map((side) => {
    const own = side < 0 ? pose.left : pose.right
    return { side, pose: own, joints: armJoints(own, side, pose.dip) }
  })

/**
 * His body in `pose`, at the context's transform (his drawing's pixels), as
 * far as his face: his shorts, his shirt and sleeves and the Codex mark, his
 * arms and his sleeves over them, and his head over all.
 */
export function drawBuddyBody(context: CanvasRenderingContext2D, head: CanvasImageSource | undefined, pose: BuddyPose, tilt = 0, faceGlass?: CanvasImageSource): void {
  const R = BUDDY_RIG
  const dip = pose.dip
  const arms = armsOf(pose)
  // His shorts, their stars kept inside them, and the seam down the middle.
  const short = shorts(dip)
  inked(context, short, R.shorts)
  context.save()
  trace(context, short)
  context.clip()
  context.fillStyle = R.star
  for (const [x, y, r, turn] of STARS) star(context, x, bodyY(y, dip), r, turn)
  context.restore()
  context.beginPath()
  context.moveTo(R.middle, 148 + dip)
  context.lineTo(R.middle, 157 + dip)
  context.lineWidth = 1.6
  context.strokeStyle = R.inkColour
  context.stroke()
  // His shirt and both sleeves as one, the folds at its sides, and the mark.
  const body = torso(dip)
  const shirt = [body, ...arms.flatMap((arm) => sleeve(arm.joints))]
  inked(context, shirt, R.shirt)
  context.beginPath()
  context.moveTo(73, bodyY(97, dip))
  context.quadraticCurveTo(72.5, bodyY(107, dip), 74, bodyY(117, dip))
  context.moveTo(114, bodyY(97, dip))
  context.quadraticCurveTo(114.5, bodyY(107, dip), 113, bodyY(117, dip))
  context.lineWidth = 1.3
  context.lineCap = 'round'
  context.stroke()
  drawCodexMark(context, R.shirtMark.x, bodyY(R.shirtMark.y, dip), R.shirtMark.radius)
  /*
   * His arms over his shirt -- a forearm curled toward you crosses his chest
   * -- but never over his shoulders, where each arm turns inside its sleeve;
   * then each sleeve over its arm again, outside his shirt, so it meets his
   * shirt with no seam.
   */
  for (const arm of arms) {
    context.save()
    context.beginPath()
    context.rect(-50, -50, 300, 320)
    shoulderOf(arm.side, dip).forEach(([x, y], i) => (i === 0 ? context.moveTo(x, y) : context.lineTo(x, y)))
    context.closePath()
    context.clip('evenodd')
    drawArm(context, arm.joints, arm.pose, arm.side)
    context.restore()
  }
  context.save()
  context.beginPath()
  context.rect(-50, -50, 300, 320)
  if (body.kind === 'poly') body.points.forEach(([x, y], i) => (i === 0 ? context.moveTo(x, y) : context.lineTo(x, y)))
  context.closePath()
  context.clip('evenodd')
  // White straight over (an inked edge clipped here would leave a grey line along his shirt), and its hem inked.
  for (const arm of arms) {
    trace(context, sleeve(arm.joints))
    context.fillStyle = R.shirt
    context.fill()
    const [a, b] = hemOf(arm.joints)
    context.beginPath()
    context.moveTo(a[0], a[1])
    context.lineTo(b[0], b[1])
    context.lineWidth = R.ink
    context.lineCap = 'round'
    context.strokeStyle = R.inkColour
    context.stroke()
  }
  context.restore()
  if (head !== undefined) {
    // His head on his neck, tilted about it.
    const neck = neckOf(dip)
    context.save()
    context.translate(neck.x, neck.y)
    context.rotate(tilt)
    context.translate(-neck.x, -neck.y)
    context.drawImage(head, 0, 0, 192, HEAD_ROWS, 0, headDrop(dip), 192, HEAD_ROWS)
    // His face is his screen: its glass in the shape of his face.
    if (faceGlass !== undefined) context.drawImage(faceGlass, 0, 0, 192, HEAD_ROWS, 0, headDrop(dip), 192, HEAD_ROWS)
    context.restore()
  }
}

/**
 * Where no arm is drawn, at each shoulder: round the joint, under the sleeve,
 * and along his shirt's edge down to the armpit -- the sleeve and his shirt
 * are what is seen there, whichever way the arm turns. The picture's left;
 * the right is its mirror about his middle.
 */
const SHOULDER: readonly (readonly [number, number])[] = [[52, 90], [57, 79], [75, 78], [77, 104], [70, 104.5], [60, 101.5], [52, 99]]

function shoulderOf(side: -1 | 1, dip: number): readonly (readonly [number, number])[] {
  return SHOULDER.map(([x, y]) => [side < 0 ? x : 2 * BUDDY_RIG.middle - x, bodyY(y, dip)] as const)
}

/** Over his body and his face: his dumbbells and fists, the farther first. */
export function drawBuddyArms(context: CanvasRenderingContext2D, pose: BuddyPose): void {
  const arms = armsOf(pose)
  for (const arm of [...arms].sort((a, b) => a.joints.hand.z - b.joints.hand.z)) {
    const near = nearness(arm.joints.hand.z)
    drawDumbbell(context, arm.joints.hand, arm.pose.grip, near)
    drawFist(context, arm.joints.hand, arm.pose.grip, near)
  }
}

/**
 * A dumbbell held at `at`, its bar rolled by `grip` (radians in the picture),
 * `scale` nearer or farther: as his maker draws one, a short bar and two round
 * plates each end, seen a little from the side -- each a disc with its lit face
 * turned out and a hub in its middle.
 */
function drawDumbbell(context: CanvasRenderingContext2D, at: Vec3, grip: number, scale: number): void {
  const R = BUDDY_RIG
  context.save()
  context.translate(at.x, at.y)
  context.rotate(grip)
  context.scale(scale * R.gear, scale * R.gear)
  const ink = R.ink / R.gear
  context.lineJoin = 'round'
  context.lineWidth = ink
  context.strokeStyle = R.inkColour
  context.beginPath()
  context.roundRect(-20, -2.5, 40, 5, 2.5)
  context.fillStyle = R.bar
  context.fill()
  context.stroke()
  const plate = (x: number, out: number, rx: number, ry: number): void => {
    context.beginPath()
    context.ellipse(x, 0, rx, ry, 0, 0, Math.PI * 2)
    context.fillStyle = R.plateColour
    context.fill()
    context.stroke()
    // Its face, turned out, lit; and the hub.
    context.beginPath()
    context.ellipse(x + out * rx * 0.32, 0, rx * 0.62, ry * 0.8, 0, 0, Math.PI * 2)
    context.fillStyle = R.plateLight
    context.fill()
    context.beginPath()
    context.ellipse(x + out * rx * 0.36, 0, rx * 0.26, ry * 0.26, 0, 0, Math.PI * 2)
    context.fillStyle = R.bar
    context.fill()
  }
  for (const end of [-1, 1]) {
    plate(end * 12.6, end, 4.4, 11.5)
    plate(end * 18.2, end, 3.8, 9.2)
  }
  context.restore()
}

function drawFist(context: CanvasRenderingContext2D, at: Vec3, grip: number, scale: number): void {
  const R = BUDDY_RIG
  context.save()
  context.translate(at.x, at.y)
  context.rotate(grip)
  context.scale(scale * R.fist, scale * R.fist)
  context.beginPath()
  context.roundRect(-6.2, -6.5, 12.4, 13, 5)
  context.fillStyle = R.skin
  context.fill()
  context.lineWidth = R.ink / R.fist
  context.strokeStyle = R.inkColour
  context.stroke()
  context.beginPath()
  for (const y of [-2.6, 0, 2.6]) {
    context.moveTo(1.5, y)
    context.lineTo(4.2, y)
  }
  context.lineWidth = 1.1
  context.stroke()
  context.restore()
}

/** His head's rows in his drawing: his cap to his chin. */
export const HEAD_ROWS = 82

/**
 * HIS HEAD, CUT FROM HIS DRAWING (row 8, column 0): his cap to his chin, his
 * shirt's collar and shoulders below his chin taken away (its white and its
 * ink beside his chin), as his shirt is drawn under it. `pixels` is the
 * drawing's RGBA, 192 wide; changed in place.
 */
export function cutHead(pixels: Uint8ClampedArray, width = 192): Uint8ClampedArray {
  const height = Math.floor(pixels.length / 4 / width)
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const i = (y * width + x) * 4
      if (y >= HEAD_ROWS) {
        pixels[i + 3] = 0
        continue
      }
      if (y < 72) continue
      const r = pixels[i] ?? 0
      const g = pixels[i + 1] ?? 0
      const b = pixels[i + 2] ?? 0
      const white = r > 205 && g > 205 && b > 205 && Math.max(r, g, b) - Math.min(r, g, b) < 26
      // Below his ears, nothing but his chin: the shirt's white anywhere, and anything at all beside his chin.
      if (white || x < 76 || x > 114) pixels[i + 3] = 0
    }
  }
  return pixels
}

/**
 * HIS FACE IS HIS SCREEN (2026-10-05). Colin: *"make the screen look like its
 * part of his face rather than layered over"*. A rounded box laid over his face
 * read as a sticker. So his screen is his face itself: the shape of it in his
 * maker's drawing -- his skin inside his face's own outline, and his eyes,
 * nose and mouth inside that (`faceOf`) -- filled with glass. His face's ink
 * is its bezel; his fringe and his ears stay over and beside it, as they are.
 */
export interface FaceShape {
  /** One byte a pixel of his head's rows (192 wide), 1 where his face is. */
  readonly mask: Uint8Array
  /** Its bounds, in his drawing's pixels. */
  readonly box: { readonly x: number; readonly y: number; readonly w: number; readonly h: number }
}

/** Where his face is searched for in his drawing, and where its filling starts: between his eyes, on his forehead and his cheeks. */
const FACE_AREA = { left: 58, top: 30, right: 122, bottom: HEAD_ROWS } as const
const FACE_SEEDS: readonly (readonly [number, number])[] = [[86, 52], [80, 66], [96, 66], [88, 74]]

const isSkin = (pixels: Uint8ClampedArray, i: number): boolean => {
  const r = pixels[i] ?? 0
  const g = pixels[i + 1] ?? 0
  const b = pixels[i + 2] ?? 0
  const a = pixels[i + 3] ?? 0
  return a > 128 && r > 150 && r - b > 60 && g > 110 && r - g > 25 && r - g < 75
}

/** His face in his drawing's RGBA (192 wide): his skin joined to his face's middle, and every hole in it (his eyes, his mouth). */
export function faceOf(pixels: Uint8ClampedArray, width = 192): FaceShape | undefined {
  const { left, top, right, bottom } = FACE_AREA
  const w = right - left
  const h = bottom - top
  const inside = (x: number, y: number): boolean => x >= left && x < right && y >= top && y < bottom
  const at = (x: number, y: number): number => (y - top) * w + (x - left)
  const face = new Uint8Array(w * h)
  const queue: number[] = []
  for (const [x, y] of FACE_SEEDS) {
    if (isSkin(pixels, (y * width + x) * 4) && face[at(x, y)] === 0) {
      face[at(x, y)] = 1
      queue.push(x, y)
    }
  }
  if (queue.length === 0) return undefined
  const steps: readonly (readonly [number, number])[] = [[1, 0], [-1, 0], [0, 1], [0, -1]]
  while (queue.length > 0) {
    const y = queue.pop() as number
    const x = queue.pop() as number
    for (const [dx, dy] of steps) {
      const nx = x + dx
      const ny = y + dy
      if (!inside(nx, ny) || face[at(nx, ny)] !== 0 || !isSkin(pixels, (ny * width + nx) * 4)) continue
      face[at(nx, ny)] = 1
      queue.push(nx, ny)
    }
  }
  // The holes in it: whatever of the area is neither his face nor reachable from its edge without crossing it.
  const outside = new Uint8Array(w * h)
  const edge: number[] = []
  for (let x = left; x < right; x += 1) edge.push(x, top, x, bottom - 1)
  for (let y = top; y < bottom; y += 1) edge.push(left, y, right - 1, y)
  for (let i = 0; i < edge.length; i += 2) {
    const x = edge[i] as number
    const y = edge[i + 1] as number
    if (face[at(x, y)] === 0 && outside[at(x, y)] === 0) {
      outside[at(x, y)] = 1
      queue.push(x, y)
    }
  }
  while (queue.length > 0) {
    const y = queue.pop() as number
    const x = queue.pop() as number
    for (const [dx, dy] of steps) {
      const nx = x + dx
      const ny = y + dy
      if (!inside(nx, ny) || face[at(nx, ny)] !== 0 || outside[at(nx, ny)] !== 0) continue
      outside[at(nx, ny)] = 1
      queue.push(nx, ny)
    }
  }
  // Out to his ink, through the soft edge where his skin blends into it (a pixel or two the skin test leaves): never into the ink.
  const isInk = (x: number, y: number): boolean => {
    const i = (y * width + x) * 4
    const lum = 0.2126 * (pixels[i] ?? 0) + 0.7152 * (pixels[i + 1] ?? 0) + 0.0722 * (pixels[i + 2] ?? 0)
    return (pixels[i + 3] ?? 0) < 128 || lum < 70
  }
  for (let pass = 0; pass < 2; pass += 1) {
    const grown: number[] = []
    for (let y = top; y < bottom; y += 1) {
      for (let x = left; x < right; x += 1) {
        if (outside[at(x, y)] === 0 || isInk(x, y)) continue
        if (steps.some(([dx, dy]) => inside(x + dx, y + dy) && outside[at(x + dx, y + dy)] === 0)) grown.push(at(x, y))
      }
    }
    for (const i of grown) outside[i] = 0
  }
  const mask = new Uint8Array(width * HEAD_ROWS)
  let x0: number = right
  let y0: number = bottom
  let x1: number = left
  let y1: number = top
  for (let y = top; y < bottom; y += 1) {
    for (let x = left; x < right; x += 1) {
      if (outside[at(x, y)] !== 0) continue
      mask[y * width + x] = 1
      x0 = Math.min(x0, x)
      y0 = Math.min(y0, y)
      x1 = Math.max(x1, x + 1)
      y1 = Math.max(y1, y + 1)
    }
  }
  return { mask, box: { x: x0, y: y0, w: x1 - x0, h: y1 - y0 } }
}

/** How his face's glass is lit: its colour at the top and the bottom. */
export interface FaceGlassLook {
  readonly top: string
  readonly bottom: string
}

/**
 * His face as glass: dark, lit a little at the top, a fine lit rim just inside
 * his face's ink, cut to the shape of his face. Drawn in his head's place,
 * under nothing but his fringe (which is not his face) -- it is drawn after his
 * head, inside his face's shape only.
 *
 * In software, as his head is (2026-10-30): it is drawn shrunk onto him at every size, and a canvas kept on the
 * graphics card shrinks badly on a Mac -- measured on the pet sheet, 2.6 to 5.1 out of 255 off the exact picture at
 * 56 to 26 px where the same pixels from a software canvas are 0.3 to 1.5 (as Orb.tsx's PaintedDown found).
 */
export function paintFaceGlass(canvas: { getContext: (kind: '2d', settings?: CanvasRenderingContext2DSettings) => CanvasRenderingContext2D | null }, face: FaceShape, look: FaceGlassLook, width = 192): boolean {
  const context = canvas.getContext('2d', { willReadFrequently: true })
  if (context === null) return false
  const { box } = face
  const ground = context.createLinearGradient(0, box.y, 0, box.y + box.h)
  ground.addColorStop(0, look.top)
  ground.addColorStop(1, look.bottom)
  context.fillStyle = ground
  context.fillRect(box.x, box.y, box.w, box.h)
  const sheen = context.createLinearGradient(0, box.y, 0, box.y + box.h * 0.55)
  sheen.addColorStop(0, 'rgba(255,255,255,0.14)')
  sheen.addColorStop(1, 'rgba(255,255,255,0)')
  context.fillStyle = sheen
  context.fillRect(box.x, box.y, box.w, box.h * 0.55)
  const image = context.getImageData(0, 0, width, HEAD_ROWS)
  const data = image.data
  const on = (x: number, y: number): boolean => x >= 0 && x < width && y >= 0 && y < HEAD_ROWS && face.mask[y * width + x] === 1
  for (let y = 0; y < HEAD_ROWS; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const i = (y * width + x) * 4
      if (!on(x, y)) {
        data[i + 3] = 0
        continue
      }
      // Just inside his face's ink: a fine lit rim.
      if (!on(x - 1, y) || !on(x + 1, y) || !on(x, y - 1) || !on(x, y + 1)) {
        data[i] = Math.min(255, (data[i] ?? 0) + 34)
        data[i + 1] = Math.min(255, (data[i + 1] ?? 0) + 40)
        data[i + 2] = Math.min(255, (data[i + 2] ?? 0) + 48)
      }
      data[i + 3] = 255
    }
  }
  context.putImageData(image, 0, 0)
  return true
}

/** Where his eyes sit on his face's glass: its middle, clear of its edges and of his fringe. */
export function eyesOn(box: FaceShape['box']): { readonly x: number; readonly y: number; readonly w: number; readonly h: number } {
  return { x: box.x + box.w * 0.12, y: box.y + box.h * 0.2, w: box.w * 0.76, h: box.h * 0.62 }
}

const FACES = new WeakMap<object, { readonly glass: CanvasImageSource; readonly box: FaceShape['box'] } | 'none'>()

/** His face's glass and its bounds, made once a sheet; undefined where his face cannot be found in it. */
export function buddyFaceGlass(sheet: CanvasImageSource, frameWidth: number, frameHeight: number, look: FaceGlassLook): { readonly glass: CanvasImageSource; readonly box: FaceShape['box'] } | undefined {
  const known = FACES.get(sheet)
  if (known !== undefined) return known === 'none' ? undefined : known
  let made: { readonly glass: CanvasImageSource; readonly box: FaceShape['box'] } | undefined
  try {
    const read = document.createElement('canvas')
    read.width = frameWidth
    read.height = frameHeight
    const context = read.getContext('2d', { willReadFrequently: true })
    if (context !== null) {
      const { row, column } = BUDDY_RIG.plate
      context.drawImage(sheet, column * frameWidth, row * frameHeight, frameWidth, frameHeight, 0, 0, frameWidth, frameHeight)
      const face = faceOf(context.getImageData(0, 0, frameWidth, frameHeight).data, frameWidth)
      if (face !== undefined) {
        const glass = document.createElement('canvas')
        glass.width = frameWidth
        glass.height = HEAD_ROWS
        if (paintFaceGlass(glass, face, look, frameWidth)) made = { glass, box: face.box }
      }
    }
  } catch {
    made = undefined
  }
  FACES.set(sheet, made ?? 'none')
  return made
}

const HEADS = new WeakMap<object, CanvasImageSource | 'none'>()

/** His head, cut once from a sheet and kept for every face wearing it; undefined where the sheet cannot be read. */
export function buddyHead(sheet: CanvasImageSource, frameWidth: number, frameHeight: number): CanvasImageSource | undefined {
  const known = HEADS.get(sheet)
  if (known !== undefined) return known === 'none' ? undefined : known
  let made: CanvasImageSource | undefined
  try {
    const canvas = document.createElement('canvas')
    canvas.width = frameWidth
    canvas.height = frameHeight
    const context = canvas.getContext('2d', { willReadFrequently: true })
    if (context !== null) {
      const { row, column } = BUDDY_RIG.plate
      context.drawImage(sheet, column * frameWidth, row * frameHeight, frameWidth, frameHeight, 0, 0, frameWidth, frameHeight)
      const image = context.getImageData(0, 0, frameWidth, frameHeight)
      cutHead(image.data, frameWidth)
      context.putImageData(image, 0, 0)
      made = canvas
    }
  } catch {
    made = undefined
  }
  HEADS.set(sheet, made ?? 'none')
  return made
}
