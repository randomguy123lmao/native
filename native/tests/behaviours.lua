-- The module's own behaviours on a standing body. Run: python host.py behaviours.lua
RESULT = ""
local failed = 0
local function check(name, ok, detail)
    if not ok then failed = failed + 1 end
    RESULT = RESULT .. (ok and "PASS  " or "FAIL  ") .. name .. (detail and ("   [" .. tostring(detail) .. "]") or "") .. "\n"
end
delirium.Init([[J:\SteamLibrary\steamapps\common\Grand Theft Auto IV\GTAIV]])
delirium.MeshAdd(1, {-30, -30, 0, 30, -30, 0, 30, 30, 0, -30, 30, 0}, {0, 1, 2, 0, 2, 3}, 0)
local function stand(x)
    local id = delirium.Create("", x, 0, 0, 0)
    delirium.Message(id, "bodyBalance", { start = true })
    for i = 1, 30 do delirium.Step(1 / 60) end
    return id
end
local function part(p, k) return p[k * 7 + 1], p[k * 7 + 2], p[k * 7 + 3] end
local function dist(ax, ay, az, bx, by, bz) return math.sqrt((ax - bx) ^ 2 + (ay - by) ^ 2 + (az - bz) ^ 2) end
local function run(id, s) for i = 1, math.floor(s * 60) do delirium.Step(1 / 60) end end
-- a point given in a part's own frame, in the world (parts: x y z, then the quaternion x y z w)
local function onPart(p, k, ox, oy, oz)
    local x, y, z, w = p[k * 7 + 4], p[k * 7 + 5], p[k * 7 + 6], p[k * 7 + 7]
    local tx, ty, tz = 2 * (y * oz - z * oy), 2 * (z * ox - x * oz), 2 * (x * oy - y * ox)
    return p[k * 7 + 1] + ox + w * tx + (y * tz - z * ty), p[k * 7 + 2] + oy + w * ty + (z * tx - x * tz), p[k * 7 + 3] + oz + w * tz + (x * ty - y * tx)
end
local info = delirium.PartInfo("")
local function radius(k) return info[k * 7 + 2] end
local function pivot(p, k) return onPart(p, k, info[k * 7 + 5], info[k * 7 + 6], info[k * 7 + 7]) end
local function norm(x, y, z) local l = math.sqrt(x * x + y * y + z * z) return x / l, y / l, z / l end
-- the body's forward (as the module works it out: from the shoulders and the spine)
local function forward(p)
    local ax, ay, az = pivot(p, 11)
    local bx, by, bz = pivot(p, 7)
    local ux, uy, uz = norm(ax - bx, ay - by, az - bz)
    ax, ay, az = pivot(p, 14)
    bx, by, bz = pivot(p, 18)
    local lx, ly, lz = ax - bx, ay - by, az - bz
    local d = lx * ux + ly * uy + lz * uz
    lx, ly, lz = norm(lx - ux * d, ly - uy * d, lz - uz * d)
    return ly * uz - lz * uy, lz * ux - lx * uz, lx * uy - ly * ux
end
check("the behaviours are listed", #delirium.Info().behaviours == 10, table.concat(delirium.Info().behaviours, " "))

-- holding the neck (a slit throat)
local a = stand(0)
local r, why = delirium.Message(a, "dlHoldWound", { start = true, part = 11, offset = { 0.1, 0, 0 } })
check("dlHoldWound starts", r ~= nil, why)
run(a, 1.5)
local p = delirium.Parts(a)
local hx, hy, hz = part(p, 20)
local nx, ny, nz = part(p, 11)
local fx, fy, fz = forward(p)
nx, ny, nz = nx + fx * 0.1, ny + fy * 0.1, nz + fz * 0.1
check("... the right hand at the neck", dist(hx, hy, hz, nx, ny, nz) < 0.08, string.format("%.2f m from the wound", dist(hx, hy, hz, nx, ny, nz)))
nx, ny, nz = part(p, 11)
check("... still standing", delirium.State(a).balancing and p[3] > 0.7, string.format("pelvis z %.2f", p[3]))
check("... listed as running", delirium.State(a).behaviours[1] == "dlHoldWound")
delirium.Message(a, "dlHoldWound", { start = false })
run(a, 1)
p = delirium.Parts(a)
hx, hy, hz = part(p, 20)
nx, ny, nz = part(p, 11)
check("... let go: the hand drops away", dist(hx, hy, hz, nx, ny, nz) > 0.25 and #delirium.State(a).behaviours == 0, string.format("%.2f m", dist(hx, hy, hz, nx, ny, nz)))

-- the left arm bleeding: the right hand holds it
local b = stand(3)
delirium.Message(b, "dlHoldWound", { start = true, part = 15 })
run(b, 1.5)
p = delirium.Parts(b)
hx, hy, hz = part(p, 20)
-- (anywhere along the forearm: from the elbow to the wrist)
local ex, ey, ez = pivot(p, 15)
local wx, wy, wz = pivot(p, 16)
local t = ((hx - ex) * (wx - ex) + (hy - ey) * (wy - ey) + (hz - ez) * (wz - ez)) / ((wx - ex) ^ 2 + (wy - ey) ^ 2 + (wz - ez) ^ 2)
t = math.min(math.max(t, 0), 1)
fx, fy, fz = ex + (wx - ex) * t, ey + (wy - ey) * t, ez + (wz - ez) * t
local touch = radius(15) + radius(20)
check("an arm wound: the other hand on it", dist(hx, hy, hz, fx, fy, fz) < touch + 0.06, string.format("%.2f m from the forearm's bone (touching at %.2f)", dist(hx, hy, hz, fx, fy, fz), touch))
local lx, ly, lz = part(p, 16)
local cx, cy, cz = part(p, 10)
check("... the hurt arm held in, its hand in front of the belly", dist(lx, ly, lz, cx, cy, cz) < 0.45, string.format("hand %.2f m from the chest", dist(lx, ly, lz, cx, cy, cz)))
check("... still standing", delirium.State(b).balancing and p[3] > 0.7, string.format("pelvis z %.2f", p[3]))

-- the fentanyl fold
local c = stand(6)
local p0 = delirium.Parts(c)
local _, _, head0 = part(p0, 12)
delirium.Message(c, "dlFentanylFold", { start = true, amount = 1, collapseAfter = 3 })
run(c, 7)
p = delirium.Parts(c)
local hdx, _, head1 = part(p, 12)
local st = delirium.State(c)
check("the fold: the head sunk forward, still on its feet", head1 < head0 - 0.15 and p[3] > 0.55, string.format("head %.2f -> %.2f m, pelvis %.2f", head0, head1, p[3]))
run(c, 5)
p = delirium.Parts(c)
check("... and deep long enough, it slumps down", p[3] < 0.45, string.format("pelvis z %.2f", p[3]))

-- brain damage: posturing (arms drawn in), then down
local d = stand(9)
p0 = delirium.Parts(d)
delirium.Message(d, "dlBrainDamage", { start = true, severity = 0.6, posture = 1 })
run(d, 2)
p = delirium.Parts(d)
local cx, cy, cz = part(p, 10)
local lx, ly, lz = part(p, 16)
local cx0, cy0, cz0 = part(p0, 10)
local lx0, ly0, lz0 = part(p0, 16)
check("brain damage, decorticate: the hand drawn up to the chest", dist(lx, ly, lz, cx, cy, cz) < dist(lx0, ly0, lz0, cx0, cy0, cz0) - 0.1,
      string.format("hand to chest %.2f -> %.2f m", dist(lx0, ly0, lz0, cx0, cy0, cz0), dist(lx, ly, lz, cx, cy, cz)))
delirium.Message(d, "dlBrainDamage", { severity = 0.95 })
run(d, 3)
p = delirium.Parts(d)
check("... worse: it can't stand", p[3] < 0.45, string.format("pelvis z %.2f", p[3]))

-- a gun held in both hands: the right hand aims (splitBody), the left holds the foregrip (dlGrip), through the recoil
local function toPart(p, k, wx, wy, wz)
    -- a world point in a part's own frame (the inverse of onPart)
    local x, y, z, w = -p[k * 7 + 4], -p[k * 7 + 5], -p[k * 7 + 6], p[k * 7 + 7]
    local ox, oy, oz = wx - p[k * 7 + 1], wy - p[k * 7 + 2], wz - p[k * 7 + 3]
    local tx, ty, tz = 2 * (y * oz - z * oy), 2 * (z * ox - x * oz), 2 * (x * oy - y * ox)
    return ox + w * tx + (y * tz - z * ty), oy + w * ty + (z * tx - x * tz), oz + w * tz + (x * ty - y * tx)
end
local e = stand(12)
p = delirium.Parts(e)
local ax, ay, az = part(p, 10)
local fx2, fy2 = forward(p)
local aim = { ax + fx2 * 10, ay + fy2 * 10, az }
r, why = delirium.Message(e, "splitBody", { start = true, aimTarget = aim, constrainHands = false, leftHandStatus = 0, rightHandStatus = 1, controllerStiffness = 8 })
check("splitBody aims", r ~= nil, why)
run(e, 1.0)
p = delirium.Parts(e)
local gx, gy, gz = part(p, 20)
local ahead0 = (gx - ax) * fx2 + (gy - ay) * fy2
-- (an SMG's foregrip: 16 cm on along the barrel from the gun hand, a little under it)
local ox, oy, oz = toPart(p, 20, gx + fx2 * 0.16, gy + fy2 * 0.16, gz - 0.03)
r, why = delirium.Message(e, "dlGrip", { start = true, hand = 16, offset = { ox, oy, oz } })
check("dlGrip starts", r ~= nil, why)
run(e, 1.0)
local function gripGap(p)
    local tx, ty, tz = onPart(p, 20, ox, oy, oz)
    local lx, ly, lz = part(p, 16)
    return dist(lx, ly, lz, tx, ty, tz)
end
p = delirium.Parts(e)
local rx, ry, rz = part(p, 20)
local cx, cy, cz = part(p, 10)
-- (near: the body's hand is held there by its muscles, softly; the ragdoll's own is put right on the grip)
check("... the left hand at the foregrip", gripGap(p) < 0.15, string.format("%.2f m from it", gripGap(p)))
check("... the gun hand out in front, aiming", (rx - cx) * fx2 + (ry - cy) * fy2 > 0.25, string.format("hand %.2f m ahead of the chest (%.2f before the grip)", (rx - cx) * fx2 + (ry - cy) * fy2, ahead0))
check("... still standing", delirium.State(e).balancing and p[3] > 0.7, string.format("pelvis z %.2f", p[3]))
local worst = 0
for shot = 1, 5 do
    delirium.Message(e, "splitBody", { fireWeapon = true })
    delirium.Impulse(e, 20, rx, ry, rz, -fx2 * 3, -fy2 * 3, 1.2)
    for i = 1, 6 do
        delirium.Step(1 / 60)
        worst = math.max(worst, gripGap(delirium.Parts(e)))
    end
end
local st = delirium.State(e)
p = delirium.Parts(e)
rx, ry, rz = part(p, 20)
cx, cy, cz = part(p, 10)
check("... a burst of five: still aiming, on its feet, the hands together", st.balancing and (rx - cx) * fx2 + (ry - cy) * fy2 > 0.2 and worst < 0.4,
      string.format("hands at most %.2f m apart from the grip, gun hand %.2f m ahead", worst, (rx - cx) * fx2 + (ry - cy) * fy2))
delirium.Message(e, "dlGrip", { start = false })
run(e, 0.5)
check("... let go", gripGap(delirium.Parts(e)) > 0.1, string.format("%.2f m", gripGap(delirium.Parts(e))))

-- on fire: GTA IV's own reaction - it goes down and writhes
local f = stand(15)
r, why = delirium.Message(f, "dlOnFire", { start = true, burning = true, health = 150 })
check("dlOnFire starts", r ~= nil, why)
run(f, 6)
p = delirium.Parts(f)
check("... on fire: down on the floor before long", p[3] < 0.5, string.format("pelvis z %.2f", p[3]))
local before = delirium.Parts(f)
run(f, 1)
local moved = 0
p = delirium.Parts(f)
for k = 0, 20 do
    local x1, y1, z1 = part(before, k)
    local x2, y2, z2 = part(p, k)
    moved = math.max(moved, dist(x1, y1, z1, x2, y2, z2))
end
check("... writhing there (not lying still)", moved > 0.05, string.format("a part moved %.2f m in a second", moved))
delirium.Message(f, "dlOnFire", { burning = false })

-- brain damage, mild: ataxia - still standing, swaying
local g = stand(18)
delirium.Message(g, "dlBrainDamage", { start = true, severity = 0.25, posture = 0 })
local sx, sy = part(delirium.Parts(g), 10)
local most = 0
for i = 1, 6 do
    run(g, 0.5)
    local cx2, cy2 = part(delirium.Parts(g), 10)
    most = math.max(most, math.sqrt((cx2 - sx) ^ 2 + (cy2 - sy) ^ 2))
end
p = delirium.Parts(g)
check("brain damage, mild: swaying but on its feet", p[3] > 0.6 and most > 0.03, string.format("pelvis z %.2f, chest swayed %.2f m", p[3], most))

-- a seizure: rigid, then jerking, then limp
local h = stand(21)
delirium.Message(h, "dlBrainDamage", { start = true, severity = 0.9, seizure = true })
run(h, 8)
-- (the most any of the head, arms and hands moves from one jerk to the next, over a second of it)
local jerk, last = 0, delirium.Parts(h)
for i = 1, 10 do
    run(h, 0.1)
    local now = delirium.Parts(h)
    for _, k in ipairs({ 12, 15, 16, 19, 20 }) do
        local x1, y1, z1 = part(last, k)
        local x2, y2, z2 = part(now, k)
        jerk = math.max(jerk, dist(x1, y1, z1, x2, y2, z2))
    end
    last = now
end
check("a seizure: jerking (the clonic phase)", jerk > 0.06, string.format("a part moved up to %.2f m in 0.1 s", jerk))
run(h, 14)
check("... then over: limp", #delirium.State(h).behaviours == 1 and not delirium.State(h).balancing, table.concat(delirium.State(h).behaviours, " "))

-- the fold told to go down: it buckles
local q2 = stand(24)
delirium.Message(q2, "dlFentanylFold", { start = true, amount = 1, collapseAfter = 0 })
run(q2, 4)
local z0 = delirium.Parts(q2)[3]
delirium.Message(q2, "dlFentanylFold", { buckle = true })
run(q2, 3)
p = delirium.Parts(q2)
check("the fold, buckling: from folded on its feet to down", z0 > 0.6 and p[3] < 0.45, string.format("pelvis z %.2f -> %.2f", z0, p[3]))
-- a lighter dose, folded less deep: it still goes down (it stood folded for good, never reaching the full fold)
local q3 = stand(27)
delirium.Message(q3, "dlFentanylFold", { start = true, amount = 0.6, collapseAfter = 2 })
run(q3, 3)
local z3 = delirium.Parts(q3)[3]
run(q3, 6)
p = delirium.Parts(q3)
check("a lighter dose: folded on its feet, then down all the same", z3 > 0.55 and p[3] < 0.45, string.format("pelvis z %.2f -> %.2f", z3, p[3]))

-- getting up: knocked down limp, lying still, then up each of Delirium's ways
-- (no trained get-up network ships any more: "learned" falls back to the muscles')
local clips = delirium.CanGetUp("")
check("GTA IV's get-up clips found", clips, tostring(clips))
local function knocked(y, back)
    local id = delirium.Create("", -12, y, 0, 0)
    delirium.Message(id, "bodyBalance", { start = true })
    run(id, 0.5)
    delirium.Message(id, "bodyRelax", { start = true, relaxation = 100 })
    local p = delirium.Parts(id)
    local cx, cy, cz = part(p, 10)
    local fx, fy = forward(p)
    local k = back and -60 or 60
    delirium.Impulse(id, 10, cx, cy, cz, fx * k, fy * k, 0)
    run(id, 4)
    return id
end
for _, way in ipairs({ { "animation", false }, { "animation", true }, { "muscles", false }, { "muscles", true } }) do
    local mode, back = way[1], way[2]
    local id = knocked(-12 + (back and 3 or 0) + (mode == "muscles" and 6 or 0), back)
    local lying = delirium.Parts(id)[3]
    delirium.Message(id, "dlGetUp", { start = true, mode = mode })
    run(id, 0.3)
    local st = delirium.State(id)
    local t = delirium.GetUpTarget(id)
    local under = st.getUp == mode and t ~= nil and #t == 21 * 7
    local done
    for i = 1, 80 do
        run(id, 0.1)
        if not delirium.State(id).getUp then done = i * 0.1 + 0.3 doneZ = delirium.Parts(id)[3] doneHead = delirium.Parts(id)[12 * 7 + 3] break end
    end
    run(id, 3)
    p = delirium.Parts(id)
    st = delirium.State(id)
    -- (the animation's get-up moves the ragdoll's bones; here, with nothing to move them, only the muscles follow: it's
    -- the muscles' get-up that has to stand the body up on its own)
    local ok = under and done ~= nil and (mode == "animation" or (st.balancing and p[3] > 0.75))
    check(string.format("dlGetUp %s from the %s", mode, back and "back" or "front"), ok,
          string.format("lying pelvis %.2f; under way %s, over after %s s (pelvis %.2f head %.2f); then pelvis %.2f, balancing %s", lying, tostring(under), tostring(done), doneZ or -1, doneHead or -1, p[3], tostring(st.balancing)))
end

-- a wound held with GTA IV's own reach (the shot's), with the free hand when the other holds a gun
local function reached(gun, y)
    local id = delirium.Create("", 9, y, 0, 0, 0.3)
    delirium.Message(id, "bodyBalance", { start = true })
    run(id, 1)
    if gun then delirium.HoldsGun(id, true, false) end
    local p = delirium.Parts(id)
    local wx, wy, wz = onPart(p, 10, 0, 0.12, 0)
    local nx, ny, nz = norm(wx - p[71], wy - p[72], wz - p[73])
    delirium.Message(id, "shot", { start = true, bodyPart = 10, hitPoint = { wx, wy, wz }, normal = { nx, ny, nz }, bulletVel = { -nx * 5, -ny * 5, -nz * 5 },
        reachForWound = true, timeBeforeReachForWound = 0.05, grabHoldTime = 10000, addShockSpin = false })
    run(id, 2)
    p = delirium.Parts(id)
    local x, y2, z = onPart(p, 10, 0, 0.12, 0)
    local st = delirium.State(id)
    return dist(x, y2, z, part(p, 16)), dist(x, y2, z, part(p, 20)), st.shot
end
local l1, r1, shot1 = reached(false, -6)
local l2, r2 = reached(true, -3)
check("a wound held with the shot's own reach (the nearer hand)", shot1 and r1 < 0.25 and r1 < l1, string.format("left %.2f, right %.2f", l1, r1))
check("... with the other hand when the gun hand is busy (HoldsGun)", l2 < 0.25 and r2 > 0.4, string.format("left %.2f, right %.2f", l2, r2))

-- a gun pointed as GTA IV's shot task points one: pointArm, the arm out straight at the target (standing, and lying on
-- the floor with nothing else holding the arms)
local function pointed(lying, y)
    local id = delirium.Create("", 15, y, 0, 0, 0.3)
    delirium.Message(id, "bodyBalance", { start = true })
    run(id, 1)
    local p = delirium.Parts(id)
    local fx, fy = forward(p)
    if lying then
        delirium.Message(id, "bodyRelax", { start = true, relaxation = 100 })
        delirium.Impulse(id, 10, p[71], p[72], p[73], -fx * 50, -fy * 50, 0)
        run(id, 4)
        p = delirium.Parts(id)
    end
    local tx, ty, tz
    if lying then
        local mx, my = (p[3 * 7 + 1] + p[6 * 7 + 1]) / 2 - p[1], (p[3 * 7 + 2] + p[6 * 7 + 2]) / 2 - p[2]
        local dx, dy = norm(mx, my, 0)
        tx, ty, tz = p[1] + dx * 6 - dy * 1.5, p[2] + dy * 6 + dx * 1.5, 1.0
    else
        tx, ty, tz = p[1] + fx * 6 + fy * 1.5, p[2] + fy * 6 - fx * 1.5, 1.3
    end
    delirium.Message(id, "pointArm", { start = true, target = { tx, ty, tz }, armStraightness = 2, twist = 0.2, pointSwingLimit = 1.0,
                                       useLeftArm = false, armStiffness = 14, armDamping = 1, useZeroPoseWhenNotPointing = true })
    run(id, 2)
    p = delirium.Parts(id)
    local sx, sy, sz = pivot(p, 18)
    local hx, hy, hz = part(p, 20)
    local reach = dist(sx, sy, sz, hx, hy, hz)
    local ax, ay, az = norm(hx - sx, hy - sy, hz - sz)
    local bx, by, bz = norm(tx - sx, ty - sy, tz - sz)
    return math.deg(math.acos(math.max(-1, math.min(1, ax * bx + ay * by + az * bz)))), reach
end
local offS, reachS = pointed(false, -9)
local offL, reachL = pointed(true, -6)
check("a gun pointed (pointArm), standing: the arm out straight at the target", offS < 8 and reachS > 0.6, string.format("%.0f deg off, reach %.2f m", offS, reachS))
check("... lying on the floor", offL < 20 and reachL > 0.6, string.format("%.0f deg off, reach %.2f m", offL, reachL))

-- walking as Delirium has it (GTA IV's player control): pushed the way it's steered, balancing, stepping
local wk = delirium.Create("", -20, 12, 0, 0, 0.3)
delirium.Message(wk, "bodyBalance", { start = true })
run(wk, 1)
local w0 = delirium.Parts(wk)
delirium.Message(wk, "dlWalk", { start = true, dir = { 0, -1, 0 }, speed = 1 })
run(wk, 4)
local w1 = delirium.Parts(wk)
check("dlWalk: walks the way it's steered, on its feet", w0[2] - w1[2] > 3 and math.abs(w1[1] - w0[1]) < 1.2 and delirium.State(wk).balancing and w1[3] > 0.75,
      string.format("%.2f m along, %.2f m aside, pelvis %.2f", w0[2] - w1[2], w1[1] - w0[1], w1[3]))
delirium.Message(wk, "dlWalk", { speed = 0 })
run(wk, 2)
local w2 = delirium.Parts(wk)
run(wk, 1)
local w3 = delirium.Parts(wk)
check("... stops when it isn't steered, still balancing", math.abs(w3[2] - w2[2]) < 0.3 and delirium.State(wk).balancing, string.format("%.2f m in the last second", math.abs(w3[2] - w2[2])))

-- kicked: staggering back the way it was kicked; a light kick caught, a hard one down
local function kicked(y, j, seed)
    local id = delirium.Create("", 18, y, 0, 0, 0.3)
    delirium.Message(id, "bodyBalance", { start = true })
    run(id, 1)
    local p = delirium.Parts(id)
    local fx, fy = forward(p)
    delirium.Message(id, "dlStagger", { start = true, dir = { -fx, -fy, 0 }, strength = j, seed = seed })
    delirium.Impulse(id, 10, p[71], p[72], p[73], -fx * j, -fy * j, 0)
    local sx, sy, low, back = p[1], p[2], 9, 0
    for i = 1, 180 do
        delirium.Step(1 / 60)
        local q = delirium.Parts(id)
        back = math.max(back, -((q[1] - sx) * fx + (q[2] - sy) * fy))
        low = math.min(low, q[3])
    end
    return back, low, delirium.State(id).balancing
end
local b1, l1, s1 = kicked(-9, 20, 3)
check("dlStagger, a kick: staggers back the way it was kicked, and catches itself", b1 > 0.5 and l1 > 0.6 and s1,
      string.format("%.2f m back, pelvis never under %.2f, balancing %s", b1, l1, tostring(s1)))
local b2, l2 = kicked(-6, 60, 1)
check("... kicked too hard to catch: down", l2 < 0.45, string.format("%.2f m back, pelvis down to %.2f", b2, l2))

-- put out: the fire's reaction ends, and nothing it started (the writhing) goes on
local fb = delirium.Create("", 18, -3, 0, 0, 0.3)
delirium.Message(fb, "bodyBalance", { start = true })
run(fb, 1)
delirium.Message(fb, "dlOnFire", { start = true, burning = true, health = 150 })
run(fb, 5)
delirium.Message(fb, "dlOnFire", { burning = false })
local over
for i = 1, 100 do
    run(fb, 0.1)
    local still = false
    for _, n in ipairs(delirium.State(fb).behaviours) do if n == "dlOnFire" then still = true end end
    if not still then over = i * 0.1 break end
end
-- (then as the Z-City part has it: balancing on its feet, or lying there)
if delirium.Parts(fb)[3] > 0.6 then delirium.Message(fb, "bodyBalance", { start = true })
else delirium.Message(fb, "bodyRelax", { start = true, relaxation = 60, mask = "fb" }) end
run(fb, 1.5)
local f0 = delirium.Parts(fb)
run(fb, 1.5)
local f1 = delirium.Parts(fb)
local moved = 0
for k = 0, 20 do
    local x1, y1, z1 = part(f0, k)
    local x2, y2, z2 = part(f1, k)
    moved = math.max(moved, dist(x1, y1, z1, x2, y2, z2))
end
check("dlOnFire, put out: over before long, and the writhing stops with it", over ~= nil and moved < 0.15,
      string.format("over after %s s; then a part moved at most %.2f m in 1.5 s", tostring(over), moved))

-- falls: from a height (GTA IV's high fall), down a staircase (the tumble), at a ledge (teetering)
do
    local pos, tri = {}, {}
    local function quad(a, b, c, d)
        local k = #pos / 3
        for _, q in ipairs({ a, b, c, d }) do pos[#pos + 1], pos[#pos + 2], pos[#pos + 3] = q[1], q[2], q[3] end
        tri[#tri + 1], tri[#tri + 2], tri[#tri + 3] = k, k + 1, k + 2
        tri[#tri + 1], tri[#tri + 2], tri[#tri + 3] = k, k + 2, k + 3
    end
    quad({ 6, 18, 2 }, { 10, 18, 2 }, { 10, 20, 2 }, { 6, 20, 2 })
    for i = 0, 9 do
        local x0, x1, z = 10 + i * 0.3, 10 + (i + 1) * 0.3, 2 - (i + 1) * 0.2
        quad({ x0, 18, z }, { x1, 18, z }, { x1, 20, z }, { x0, 20, z })
        quad({ x0, 18, z + 0.2 }, { x0, 18, z }, { x0, 20, z }, { x0, 20, z + 0.2 })
    end
    quad({ -10, 18, 2 }, { -6, 18, 2 }, { -6, 22, 2 }, { -10, 22, 2 })
    quad({ -6, 18, 2 }, { -6, 18, 0 }, { -6, 22, 0 }, { -6, 22, 2 })
    delirium.MeshAdd(3, pos, tri, 0)
end
local function sane(p) for _, x in ipairs(p) do if x ~= x or math.abs(x) == math.huge then return false end end return true end
local hf = delirium.Create("", 0, 24, 8, 0, 0.3)
delirium.HighFall(hf, "*")
local hp0, hmove = delirium.Parts(hf), 0
local hlast = hp0
for i = 1, 60 do
    delirium.Step(1 / 60)
    local q = delirium.Parts(hf)
    for _, k in ipairs({ 16, 20 }) do
        hmove = hmove + dist(q[k * 7 + 1] - q[1], q[k * 7 + 2] - q[2], q[k * 7 + 3] - q[3], hlast[k * 7 + 1] - hlast[1], hlast[k * 7 + 2] - hlast[2], hlast[k * 7 + 3] - hlast[3])
    end
    hlast = q
end
run(hf, 3)
local hp = delirium.Parts(hf)
check("HighFall: from 8 m, the arms windmilling on the way down, landed sound", hmove > 1.5 and hp[3] < 1.2 and sane(hp), string.format("arms moved %.2f m against the body in 1 s, pelvis now %.2f", hmove, hp[3]))
local sd = delirium.Create("", 9.2, 19, 2, 0, 0.3)
delirium.Message(sd, "bodyBalance", { start = true })
run(sd, 1)
local sp = delirium.Parts(sd)
delirium.Message(sd, "stopAllBehaviours", {})
delirium.Impulse(sd, 10, sp[71], sp[72], sp[73], 70, 0, 0)
run(sd, 0.3)
check("rollDownStairs taken", delirium.Message(sd, "rollDownStairs", { start = true }) ~= nil)
run(sd, 4)
sp = delirium.Parts(sd)
check("... down the stairs to the bottom", sp[1] > 12.5 and sp[3] < 0.6 and sane(sp), string.format("pelvis x %.2f z %.2f", sp[1], sp[3]))
local tt = delirium.Create("", -6.5, 20, 2, 0, 0.3)
delirium.Message(tt, "bodyBalance", { start = true })
run(tt, 1)
check("teeter taken", delirium.Message(tt, "teeter", { start = true, edgeLeft = { -6, 21, 2 }, edgeRight = { -6, 19, 2 } }) ~= nil)
run(tt, 3)
local tp = delirium.Parts(tt)
check("... at a 2 m ledge: kept back from it, on its feet", tp[1] < -6.1 and tp[3] > 2.6 and delirium.State(tt).balancing, string.format("pelvis x %.2f z %.2f", tp[1], tp[3]))

delirium.Destroy(tt)

-- an NPC's body knocked back off the ledge: it grabs the lip (GTA IV's grab, along it) and hangs from it
local lg = delirium.Create("", -6.35, 19.0, 2, 0, 0.3)
delirium.Message(lg, "bodyBalance", { start = true })
run(lg, 1)
local lp = delirium.Parts(lg)
delirium.Impulse(lg, 10, lp[71], lp[72], lp[73], 60, 0, 0)
local lsent, lgrip, lhung = nil, nil, 0
for i = 1, 60 * 3 do
    delirium.Step(1 / 60)
    lp = delirium.Parts(lg)
    if not lsent and lp[1] > -6.05 then
        check("grab taken (along the lip)", delirium.Message(lg, "grab", { start = true, linegrab = true, useLeft = true, useRight = true, dontLetGo = true,
                                                                           pos1 = { -6, 18.6, 2 }, pos2 = { -6, 19.4, 2 } }) ~= nil)
        lsent = i
    end
    local st = delirium.State(lg)
    if lsent and (st.gripLeft or st.gripRight) then
        lgrip = lgrip or (i - lsent) / 60
        lhung = lhung + 1
    end
end
local lst = delirium.State(lg)
check("... a hand on it soon, and hanging there (the pelvis under the lip, off the floor)",
      lgrip ~= nil and lgrip < 0.8 and lst.grabbing and (lst.gripLeft or lst.gripRight) and lp[3] < 1.7 and lp[3] > 0.4 and lp[1] > -6,
      string.format("first grip %s s, held %.1f s, pelvis x %.2f z %.2f", tostring(lgrip), lhung / 60, lp[1], lp[3]))
delirium.Message(lg, "grab", { start = false })
run(lg, 1.5)
lp = delirium.Parts(lg)
check("... let go: it drops", not delirium.State(lg).grabbing and lp[3] < 0.4, string.format("pelvis z %.2f", lp[3]))

-- knocked back into a railing at hip height: over it (fallOverWall), the push stopped once it's over the top
do
    local pos, tri = {}, {}
    local function quad(a, b, c, d)
        local k = #pos / 3
        for _, q in ipairs({ a, b, c, d }) do pos[#pos + 1], pos[#pos + 2], pos[#pos + 3] = q[1], q[2], q[3] end
        tri[#tri + 1], tri[#tri + 2], tri[#tri + 3] = k, k + 1, k + 2
        tri[#tri + 1], tri[#tri + 2], tri[#tri + 3] = k, k + 2, k + 3
    end
    local x0, x1, y0, y1, h = 25, 25.15, -28, -22, 0.8
    quad({ x0, y0, h }, { x1, y0, h }, { x1, y1, h }, { x0, y1, h })
    quad({ x0, y0, 0 }, { x0, y0, h }, { x0, y1, h }, { x0, y1, 0 })
    quad({ x1, y0, 0 }, { x1, y1, 0 }, { x1, y1, h }, { x1, y0, h })
    quad({ x0, y0, 0 }, { x1, y0, 0 }, { x1, y0, h }, { x0, y0, h })
    quad({ x0, y1, 0 }, { x0, y1, h }, { x1, y1, h }, { x1, y1, 0 })
    delirium.MeshAdd(4, pos, tri, 0)
end
local fw = delirium.Create("", 24.45, -25, 0, 0, 0.3)
delirium.Message(fw, "bodyBalance", { start = true })
run(fw, 1)
local fp = delirium.Parts(fw)
delirium.Impulse(fw, 10, fp[71], fp[72], fp[73], 70, 0, 0)
local fsent, fstop
for i = 1, 60 * 3 do
    delirium.Step(1 / 60)
    fp = delirium.Parts(fw)
    if not fsent and fp[1] > 24.65 then
        check("fallOverWall taken", delirium.Message(fw, "fallOverWall", { start = true, useSpecifiedPoint = true, fallOverPoint = { 25, -25, 0.8 },
                                                                           fallOverEdge = { 0, 1, 0 }, magOfForce = 1.5, maxDistanceFromPelToHitPoint = 0.6 }) ~= nil)
        fsent = i
    end
    if fsent and not fstop and fp[1] > 25.35 then
        delirium.Message(fw, "fallOverWall", { start = false })
        fstop = i
    end
end
check("... over the railing, down on the far side", fstop ~= nil and fp[1] > 25.3 and fp[3] < 0.5 and sane(fp), string.format("pelvis x %.2f z %.2f", fp[1], fp[3]))

-- electrocuted (a taser, a live wire): jolt after jolt while it lasts, then over
local ec = delirium.Create("", -25, -25, 0, 0, 0.3)
delirium.Message(ec, "bodyBalance", { start = true })
run(ec, 1)
check("electrocute taken", delirium.Message(ec, "electrocute", { start = true }) ~= nil)
local elast, emost = delirium.Parts(ec), 0
local eon = false
for i = 1, 90 do
    delirium.Step(1 / 60)
    local q = delirium.Parts(ec)
    for _, k in ipairs({ 12, 16, 20 }) do
        emost = math.max(emost, dist(q[k * 7 + 1], q[k * 7 + 2], q[k * 7 + 3], elast[k * 7 + 1], elast[k * 7 + 2], elast[k * 7 + 3]))
    end
    elast = q
end
for _, n in ipairs(delirium.State(ec).extras) do if n == "electrocute" then eon = true end end
check("... jolting (its head and hands jerked about), sound", eon and emost > 0.008 and sane(elast), string.format("most a part moved in a step %.3f m", emost))
delirium.Message(ec, "electrocute", { start = false })
run(ec, 0.1)
local eoff = true
for _, n in ipairs(delirium.State(ec).extras) do if n == "electrocute" then eoff = false end end
check("... and over when it's stopped", eoff)

-- hit by a car: thrown, tucked and rolling (bodyRollUp), then catching itself
local cr = delirium.Create("", -25, 25, 0, 0, 0.3)
delirium.Message(cr, "bodyBalance", { start = true })
run(cr, 1)
local cp = delirium.Parts(cr)
delirium.Impulse(cr, 0, cp[1], cp[2], cp[3], 600, 0, 120)
check("bodyRollUp taken", delirium.Message(cr, "bodyRollUp", { start = true }) ~= nil)
run(cr, 1.2)
delirium.Message(cr, "bodyRollUp", { start = false })
delirium.CatchFall(cr, "*")
run(cr, 2)
cp = delirium.Parts(cr)
check("... thrown, rolled, down and sound", cp[3] < 0.5 and sane(cp) and cp[1] > -24, string.format("pelvis x %.2f z %.2f", cp[1], cp[3]))

-- choked, lying: the legs thrash (bodyWrithe, legs only) - and stop when it's over
local wr = delirium.Create("", 25, 25, 0, 0, 0.3)
delirium.Message(wr, "bodyRelax", { start = true, relaxation = 100 })
run(wr, 2)
local function feet(id, s)
    local last, moved = delirium.Parts(id), 0
    for i = 1, math.floor(s * 60) do
        delirium.Step(1 / 60)
        local q = delirium.Parts(id)
        for _, f in ipairs({ 3, 6 }) do
            moved = moved + dist(q[f * 7 + 1] - q[1], q[f * 7 + 2] - q[2], q[f * 7 + 3] - q[3], last[f * 7 + 1] - last[1], last[f * 7 + 2] - last[2], last[f * 7 + 3] - last[3])
        end
        last = q
    end
    return moved
end
delirium.Message(wr, "stopAllBehaviours", {})
check("bodyWrithe (legs) taken", delirium.Message(wr, "bodyWrithe", { start = true, mask = "lb", legStiffness = 10, legPeriod = 0.45, legAmplitude = 1.6, kneeAmplitude = 1.4 }) ~= nil)
run(wr, 0.5)
local thrash = feet(wr, 1.5)
delirium.Message(wr, "bodyWrithe", { start = false })
run(wr, 1)
local after = feet(wr, 1.5)
check("... the legs thrash, and stop when it's stopped", thrash > 1 and after < thrash * 0.25, string.format("feet moved %.2f m, then %.2f m", thrash, after))

-- in severe pain, lying: curled up (bodyFoetal), a hand still on its belly wound (the wound hold keeps the arms)
local fo = delirium.Create("", 25, 19, 0, 0, 0.3)
delirium.Message(fo, "bodyRelax", { start = true, relaxation = 100 })
run(fo, 2)
delirium.Message(fo, "stopAllBehaviours", {})
local function curlOf(id)
    local q = delirium.Parts(id)
    return (dist(q[71], q[72], q[73], q[15], q[16], q[17]) + dist(q[71], q[72], q[73], q[36], q[37], q[38])) / 2
end
local fc0 = curlOf(fo)
delirium.Message(fo, "clutchWound", { start = true, numInjuries = 1, injury1Component = 8, injury1LocalPosition = { 0.1, 0, 0 }, injury1LocalNormal = { 0.1, 0, 0 } })
check("bodyFoetal taken", delirium.Message(fo, "bodyFoetal", { start = true, randomSeed = 7 }) ~= nil)
run(fo, 2.5)
local fc1 = curlOf(fo)
local fq = delirium.Parts(fo)
local fwx, fwy, fwz = onPart(fq, 8, 0.1, 0, 0)
local fhand = math.min(dist(fwx, fwy, fwz, fq[113], fq[114], fq[115]), dist(fwx, fwy, fwz, fq[141], fq[142], fq[143]))
check("... curled up (knees to the chest: a body lying straight is about 1.1 m), the hand still on the wound, sound", fc1 < 0.68 and fc1 <= fc0 and fhand < 0.35 and sane(fq),
      string.format("knees-chest %.2f -> %.2f m, hand %.2f m from the wound", fc0, fc1, fhand))

-- (its own function: the file's main chunk is at Lua's limit of locals)
;(function()
-- a bullet, by where it lands (dlShotReact, over GTA IV's own shot): each on a body standing at rest
local function standAt(x, y)
    local id = delirium.Create("", x, y, 0, 0)
    delirium.Message(id, "bodyBalance", { start = true })
    run(id, 0.5)
    return id
end
-- (level: the way the shoulders face; how far the trunk leans that way, of its length; the head's height over the neck)
local function facing(p)
    local ax, ay = pivot(p, 14)
    local bx, by = pivot(p, 18)
    local lx, ly = ax - bx, ay - by
    local l = math.sqrt(lx * lx + ly * ly)
    return ly / l, -lx / l
end
local function lean(p)
    local fx, fy = facing(p)
    local dx, dy, dz = p[11 * 7 + 1] - p[1], p[11 * 7 + 2] - p[2], p[11 * 7 + 3] - p[3]
    return (dx * fx + dy * fy) / math.sqrt(dx * dx + dy * dy + dz * dz)
end
local function yawOf(p) local fx, fy = facing(p) return math.deg(math.atan2(fy, fx)) end
local function turned(a0, a1) local d = (a1 - a0 + 180) % 360 - 180 return math.abs(d) end

local gut0 = standAt(-28, -18)
local gutB = standAt(-24, -18)
local p0 = delirium.Parts(gut0)
local fx, fy = facing(p0)
check("dlShotReact taken (a gut shot)", delirium.Message(gut0, "dlShotReact", { start = true, region = "gut", side = 1, dir = { -fx, -fy, 0 }, power = 1 }) ~= nil)
run(gut0, 0.7)
run(gutB, 0.7)
local lg0, lgB = lean(delirium.Parts(gut0)), lean(delirium.Parts(gutB))
check("... shot in the gut: doubled over, the trunk leaning forward (against one not shot)", lg0 > lgB + 0.15 and delirium.State(gut0).balancing,
      string.format("lean %.2f, not shot %.2f", lg0, lgB))
-- (upright again: on average over its last two seconds - a standing body sways and nods)
local track, late, stood = {}, 0, true
for i = 1, 8 do
    run(gut0, 0.5)
    local l = lean(delirium.Parts(gut0))
    track[#track + 1] = string.format("%.2f", l)
    if i > 4 then late = late + l / 4 end
    stood = stood and delirium.State(gut0).balancing
end
check("... and over by itself, upright again", #delirium.State(gut0).behaviours == 0 and late < lgB + 0.15 and stood,
      string.format("%s; lean every 0.5 s after: %s", table.concat(delirium.State(gut0).behaviours, " "), table.concat(track, " ")))

local sh = standAt(-20, -18)
local sp0 = delirium.Parts(sh)
fx, fy = facing(sp0)
local y0 = yawOf(sp0)
delirium.Message(sh, "dlShotReact", { start = true, region = "shoulder", side = 1, dir = { -fx, -fy, 0 }, power = 1 })
run(sh, 0.4)
local spun = turned(y0, yawOf(delirium.Parts(sh)))
check("dlShotReact, the right shoulder: spun round by it", spun > 12, string.format("turned %.0f degrees", spun))

local hd = standAt(-16, -18)
-- (the head ahead of the neck, along the way the shoulders face: hanging forward, it's well ahead)
local function headAhead(p)
    local hx, hy = facing(p)
    return (p[12 * 7 + 1] - p[11 * 7 + 1]) * hx + (p[12 * 7 + 2] - p[11 * 7 + 2]) * hy
end
local ha0 = headAhead(delirium.Parts(hd))
fx, fy = facing(delirium.Parts(hd))
delirium.Message(hd, "dlShotReact", { start = true, region = "head", side = 1, dir = { -fx, -fy, 0 }, power = 1 })
run(hd, 0.15)
local ha1 = headAhead(delirium.Parts(hd))
run(hd, 0.85)
local ha2 = headAhead(delirium.Parts(hd))
check("dlShotReact, the head (not killed): snapped back, then hanging forward, stunned", ha1 < ha0 and ha2 > ha0 + 0.03,
      string.format("head %.3f m ahead of the neck, %.3f snapped back, %.3f hanging", ha0, ha1, ha2))

-- a leg shot: nothing of the reaction's own (the leg giving is the injury's and GTA IV's) - no lean left behind
local function tiltLeft(p)
    local fx, fy = facing(p)
    local lx, ly = -fy, fx
    local dx, dy, dz = p[11 * 7 + 1] - p[1], p[11 * 7 + 2] - p[2], p[11 * 7 + 3] - p[3]
    return (dx * lx + dy * ly) / math.sqrt(dx * dx + dy * dy + dz * dz)
end
local lg2 = standAt(-28, -22)
fx, fy = facing(delirium.Parts(lg2))
local tilt0, worst = tiltLeft(delirium.Parts(lg2)), 0
delirium.Message(lg2, "dlShotReact", { start = true, region = "leg", side = 0, dir = { -fx, -fy, 0 }, power = 1 })
for i = 1, 6 do run(lg2, 0.5) worst = math.max(worst, math.abs(tiltLeft(delirium.Parts(lg2)) - tilt0)) end
-- (a body left alone sways about 0.04; a lean left on it, 0.2 to 0.3)
check("dlShotReact, a leg: no lean of its own left behind, standing steady", worst < 0.12 and delirium.State(lg2).balancing and #delirium.State(lg2).behaviours == 0,
      string.format("tilted at most %.2f", worst))
-- hit again while it plays out (the gut, then an arm): the second takes over; upright again after, the pose let go of
local tw = standAt(-24, -22)
local tw0 = lean(delirium.Parts(tw))
fx, fy = facing(delirium.Parts(tw))
delirium.Message(tw, "dlShotReact", { start = true, region = "gut", side = 1, dir = { -fx, -fy, 0 }, power = 1 })
run(tw, 0.5)
local twIn = lean(delirium.Parts(tw))
delirium.Message(tw, "dlShotReact", { start = true, region = "arm", side = 1, dir = { -fx, -fy, 0 }, power = 1 })
run(tw, 0.6)
local twArm = lean(delirium.Parts(tw))
local twl = {}
for i = 1, 6 do run(tw, 0.5) twl[i] = lean(delirium.Parts(tw)) end
local twEnd = (twl[3] + twl[4] + twl[5] + twl[6]) / 4
check("hit in the gut, then the arm: the arm's takes over (the trunk let go of), upright after", twIn > tw0 + 0.2 and twArm < twIn - 0.1
      and twEnd < tw0 + 0.15 and #delirium.State(tw).behaviours == 0 and delirium.State(tw).balancing,
      string.format("lean %.2f, gut %.2f, then arm %.2f, after %.2f", tw0, twIn, twArm, twEnd))
-- folded over (fentanyl), shot: the fold's pose back at once when the shot's reaction is over (they share the one pose)
local fz = standAt(-20, -22)
delirium.Message(fz, "dlFentanylFold", { start = true, amount = 1, collapseAfter = 0 })
run(fz, 5)
local fz0 = lean(delirium.Parts(fz))
fx, fy = facing(delirium.Parts(fz))
delirium.Message(fz, "dlShotReact", { start = true, region = "neck", side = 1, dir = { -fx, -fy, 0 }, power = 1 })
run(fz, 3)
local fz1 = lean(delirium.Parts(fz))
local still = false
for _, n in ipairs(delirium.State(fz).behaviours) do if n == "dlFentanylFold" then still = true end end
check("folded over, shot: folded still once the shot's reaction is over", still and fz0 > 0.4 and fz1 > fz0 - 0.15,
      string.format("lean %.2f folded, %.2f after", fz0, fz1))

-- a hurt leg (the engine's injury): it gives under the weight
local function kneeBend(p, hip, knee, ankle)
    local ax, ay, az = pivot(p, hip)
    local bx, by, bz = pivot(p, knee)
    local cx, cy, cz = pivot(p, ankle)
    local ux, uy, uz = norm(ax - bx, ay - by, az - bz)
    local vx, vy, vz = norm(cx - bx, cy - by, cz - bz)
    return math.deg(math.acos(math.max(-1, math.min(1, ux * vx + uy * vy + uz * vz))))
end
local lh = standAt(-4, -18)
local lhz0 = delirium.Parts(lh)[3]
check("an injury taken (the left leg, badly)", delirium.Message(lh, "injury", { mask = "ll", amount = 0.9 }) ~= nil)
run(lh, 1.5)
local lp = delirium.Parts(lh)
local kl, kr = kneeBend(lp, 1, 2, 3), kneeBend(lp, 4, 5, 6)
check("... the leg gives: that knee bent more than the other, or down on it", kl < kr - 8 or lp[3] < lhz0 - 0.15,
      string.format("knees %.0f (hurt) and %.0f degrees straight, pelvis %.2f -> %.2f m", kl, kr, lhz0, lp[3]))
delirium.Message(lh, "injury", { mask = "ll", amount = 0 })

-- thrown at a wall (dlBrace): both hands out onto it
local wx = 4 + 0.6
delirium.MeshAdd(6, { wx, -19.5, 0, wx, -16.5, 0, wx, -16.5, 2.5, wx, -19.5, 2.5 }, { 0, 2, 1, 0, 3, 2 }, 0)
local br = standAt(4, -18)
local bp = delirium.Parts(br)
fx, fy = facing(bp)
check("(the braced one faces the wall)", fx > 0.9, string.format("facing %.2f %.2f", fx, fy))
check("dlBrace taken", delirium.Message(br, "dlBrace", { start = true, pos = { wx, -18, 1.3 }, normal = { -1, 0, 0 } }) ~= nil)
-- (the nearest each hand comes to the wall, and how high it is then)
local near = { { 9, 0 }, { 9, 0 } }
for i = 1, 42 do
    delirium.Step(1 / 60)
    bp = delirium.Parts(br)
    for k, part in ipairs({ 16, 20 }) do
        local gap = wx - bp[part * 7 + 1]
        if gap < near[k][1] then near[k] = { gap, bp[part * 7 + 3] } end
    end
end
check("... both hands out on the wall, at the shoulders' height", near[1][1] < 0.15 and near[2][1] < 0.15 and near[1][2] > 1.0 and near[2][2] > 1.0 and sane(bp),
      string.format("hands within %.2f and %.2f m of the wall, at %.2f and %.2f m", near[1][1], near[2][1], near[1][2], near[2][2]))
run(br, 1.5)
check("... and over by itself", #delirium.State(br).behaviours == 0, table.concat(delirium.State(br).behaviours, " "))
delirium.MeshRemove(6)
-- the right hand busy (a gun in it): braced with the left only
local wx2 = 12 + 0.6
delirium.MeshAdd(7, { wx2, -19.5, 0, wx2, -16.5, 0, wx2, -16.5, 2.5, wx2, -19.5, 2.5 }, { 0, 2, 1, 0, 3, 2 }, 0)
local b1 = standAt(12, -18)
delirium.Message(b1, "dlBrace", { start = true, pos = { wx2, -18, 1.3 }, normal = { -1, 0, 0 }, useRight = false })
local near1 = { 9, 9 }
for i = 1, 42 do
    delirium.Step(1 / 60)
    local q = delirium.Parts(b1)
    near1[1] = math.min(near1[1], wx2 - q[16 * 7 + 1])
    near1[2] = math.min(near1[2], wx2 - q[20 * 7 + 1])
end
check("the right hand busy (useRight false): braced with the left only", near1[1] < 0.15 and near1[2] > 0.3,
      string.format("left within %.2f m of the wall, right %.2f", near1[1], near1[2]))
check("... ended as the body goes limp (restore=false): taken, and over", delirium.Message(b1, "dlBrace", { start = false, restore = false }) ~= nil
      and #delirium.State(b1).behaviours == 0)
delirium.MeshRemove(7)
end)()

-- holding a wound (clutchWound): the hand on it - the hand's centre where the palm rests on the skin there, not a
-- hand's length short of it. A wound on the chest, standing.
;(function()
    local cw = delirium.Create("", -16, -26, 0, 0)
    delirium.Message(cw, "bodyBalance", { start = true })
    run(cw, 0.5)
    local q = delirium.Parts(cw)
    local function pv(k) return q[k * 7 + 1], q[k * 7 + 2], q[k * 7 + 3] end
    local ax, ay, az = pv(14)
    local bx, by, bz = pv(18)
    local nx, ny, nz = pv(11)
    local px, py, pz = pv(0)
    local ux, uy, uz = norm(nx - px, ny - py, nz - pz)
    local lx, ly, lz = ax - bx, ay - by, az - bz
    local d = lx * ux + ly * uy + lz * uz
    lx, ly, lz = norm(lx - ux * d, ly - uy * d, lz - uz * d)
    local fx, fy, fz = ly * uz - lz * uy, lz * ux - lx * uz, lx * uy - ly * ux
    -- (the front of the chest, a little to the left: as a world offset, into the chest part's own frame)
    local wx, wy, wz = norm(fx + lx * 0.6, fy + ly * 0.6, fz + lz * 0.6)
    local r = radius(10)
    wx, wy, wz = wx * r, wy * r, wz * r
    local qx, qy, qz, qw = -q[10 * 7 + 4], -q[10 * 7 + 5], -q[10 * 7 + 6], q[10 * 7 + 7]
    local tx, ty, tz = 2 * (qy * wz - qz * wy), 2 * (qz * wx - qx * wz), 2 * (qx * wy - qy * wx)
    local lp = { wx + qw * tx + (qy * tz - qz * ty), wy + qw * ty + (qz * tx - qx * tz), wz + qw * tz + (qx * ty - qy * tx) }
    delirium.Message(cw, "clutchWound", { start = true, numInjuries = 1, injury1Component = 10, injury1LocalPosition = lp, injury1LocalNormal = lp })
    run(cw, 1.5)
    q = delirium.Parts(cw)
    local wx2, wy2, wz2 = onPart(q, 10, lp[1], lp[2], lp[3])
    local best = math.min(dist(wx2, wy2, wz2, q[16 * 7 + 1], q[16 * 7 + 2], q[16 * 7 + 3]), dist(wx2, wy2, wz2, q[20 * 7 + 1], q[20 * 7 + 2], q[20 * 7 + 3]))
    check("clutchWound, a chest wound standing: the hand on it (its centre within a palm's thickness and a little of it)", best < 0.10 and delirium.State(cw).balancing,
          string.format("the hand's centre %.3f m from the wound", best))
    delirium.Message(cw, "clutchWound", { start = false })
end)()

check("an unknown behaviour is refused", delirium.Message(a, "dlNothing", { start = true }) == nil)
RESULT = RESULT .. (failed == 0 and "all passed" or (failed .. " FAILED")) .. "\n"
