-- The Delirium addon's core (its real Lua) on the real module, with gmod_stub.lua standing in for the game. The game's
-- physics isn't here (the module's side of it: examples/hostsim.rs): what's checked is the core's own part - the
-- model's profile, which part each bone carries and how it's turned, and poses both ways.
-- Run: python host.py gmod_stub.lua core.lua
STUB_DM = include("delirium/dl_math.lua")
local DM = STUB_DM
local failed = 0
local function check(name, ok, detail)
    if not ok then failed = failed + 1 end
    OUT((ok and "PASS  " or "FAIL  ") .. name .. (detail and ("   [" .. tostring(detail) .. "]") or ""))
end

include("delirium/sv_delirium.lua")
local D = Delirium
check("the core loaded", D ~= nil and D.Attach ~= nil)
local found = delirium.FindGTA()
SET_CVAR("delirium_gta", found or [[J:\SteamLibrary\steamapps\common\Grand Theft Auto IV\GTAIV]])
local hostOk, hostWhy = delirium.HostReady()
check("no game physics here: the module says so (before starting too)", hostOk == false and string.find(tostring(hostWhy), "vphysics") ~= nil, hostWhy)
local ok, why = D.Start()
check("without it, Delirium won't start", not ok and string.find(tostring(why), "VPhysics") ~= nil, why)
D.noHostCheck, D.retryAt, D.failed = true, nil, nil
ok, why = D.Start()
check("started (the check skipped for the test)", ok, why)

local MODEL = "models/test/male.mdl"
MAKE_MODEL(MODEL, 1.05, 35)
local prof = D.Profile(MODEL)
check("a profile for the model, the body fitted to it", prof ~= nil and prof.fitted)
local mapped = 0
for _ in pairs(prof.map) do mapped = mapped + 1 end
check("15 physics bones, each carrying a part", mapped == 15 and prof.count == 15, mapped)
local between = {}
for _, w in ipairs(prof.between) do between[#between + 1] = w.part end
table.sort(between)
check("the six parts with no bone put between (lower spine, neck, clavicles)", table.concat(between, " ") == "7 8 9 11 13 17", table.concat(between, " "))
check("handed to the module: 6 numbers a bone, 4 a part between", #prof.hostBones == 90 and #prof.hostBetween == 24, #prof.hostBones .. " " .. #prof.hostBetween)
-- the bones' parents for the muscles: the spine's goes to the pelvis, the head's to the chest
local function boneOf(part) return prof.partBone[part] end
local function parentOf(i) return prof.hostBones[i * 6 + 2] end
check("the chest's muscle pulls against the pelvis", parentOf(boneOf(10)) == boneOf(0), parentOf(boneOf(10)))
check("the head's against the chest, the upper arm's against the chest", parentOf(boneOf(12)) == boneOf(10) and parentOf(boneOf(14)) == boneOf(10))
check("the pelvis has none", parentOf(boneOf(0)) == -1)

-- a bone's part is the bone turned by its offset: as the bone matching has it, at any angle
local worst = 0
for i, m in pairs(prof.map) do
    for _, ang in ipairs({ Angle(0, 0, 0), Angle(30, 70, -20), Angle(-80, 200, 45), Angle(10, -100, 170) }) do
        local a = DM.qmul(D.QuatOf(ang), m.offset)
        local f, r, u = ang:Forward(), ang:Right(), ang:Up()
        local b = DM.inverseRetarget(m.eu, m.src, DM.v(f.x, f.y, f.z), DM.v(-r.x, -r.y, -r.z), DM.v(u.x, u.y, u.z))
        local dot = math.abs(a.x * b.x + a.y * b.y + a.z * b.z + a.w * b.w)
        worst = math.max(worst, math.deg(2 * math.acos(math.min(dot, 1))))
    end
end
check("bone * offset = the bone matching's part, at any angle", worst < 0.05, string.format("%.3f deg", worst))

-- poses both ways: a ragdoll stood up in the body's pose gives that pose back
local id = delirium.Create(prof.key, 2, 1, 0, 0.7, 0.3)
local frames = DM.unpackFrames(delirium.Parts(id))
local rag = ents.Create("prop_ragdoll")
rag:SetModel(MODEL)
rag:SetPos(Vector(0, 0, 0))
rag:Spawn()
local c = { ent = rag, profile = prof }
D.PoseRagdoll(c, frames)
local pose = D.PoseOf(c, rag)
local worstP, worstA, at = 0, 0, ""
for k = 0, 20 do
    local o = k * 13
    local p = frames[k].p
    local dp = math.sqrt((pose[o + 1] - p.x) ^ 2 + (pose[o + 2] - p.y) ^ 2 + (pose[o + 3] - p.z) ^ 2)
    local q = frames[k].q
    local dot = math.abs(pose[o + 4] * q.x + pose[o + 5] * q.y + pose[o + 6] * q.z + pose[o + 7] * q.w)
    local da = math.deg(2 * math.acos(math.min(dot, 1)))
    if dp > worstP then worstP, at = dp, k end
    worstA = math.max(worstA, da)
end
check("the body's pose round the ragdoll and back", worstP < 0.02 and worstA < 2, string.format("%.3f m (part %s), %.2f deg", worstP, at, worstA))
local fp = rag:GetPhysicsObjectNum(prof.byName[DM.B .. "L_Foot"].index):GetPos()
check("stood up with its feet on the floor", fp.z > -2 and fp.z < 12, string.format("left foot z %.1f units", fp.z))

-- taking a ragdoll with no game physics to drive it: refused, saying why
local c2, err = D.Attach(rag, { react = "balance" })
check("without the game's physics a ragdoll isn't taken, and it says why", c2 == nil and string.find(tostring(err), "vphysics") ~= nil, err)
check("nothing left half-made", next(D.chars) == nil and delirium.Count() == 1, delirium.Count())

-- getting up, an NPC's body: the get-up played through the core's own Lua (the stand-in ragdoll follows the bones' targets
-- exactly), and the NPC back the moment it's up - not left standing on its own first, waiting to be still
delirium.MeshAdd(1, {-30, -30, 0, 30, -30, 0, 30, 30, 0, -30, 30, 0}, {0, 1, 2, 0, 2, 3}, 0)
local restored
local realRestore = D.RestoreNpc
D.RestoreNpc = function(cc) restored = { at = CurTime(), held = cc.holdPose ~= nil, rising = cc.rising } cc.restoring = true end
local function knocked(y, key)
    local b = delirium.Create(key, -10, y, 0, 0, 0.3)
    delirium.Message(b, "bodyBalance", { start = true })
    for i = 1, 30 do delirium.Step(1 / 60) end
    delirium.Message(b, "bodyRelax", { start = true, relaxation = 100 })
    local p = delirium.Parts(b)
    -- (pushed over forwards: from its hips, left to right, turned up)
    local lx, ly = p[1 * 7 + 1] - p[4 * 7 + 1], p[1 * 7 + 2] - p[4 * 7 + 2]
    local l = math.sqrt(lx * lx + ly * ly)
    delirium.Impulse(b, 10, p[71], p[72], p[73], ly / l * 60, -lx / l * 60, 0)
    for i = 1, 240 do delirium.Step(1 / 60) end
    return b
end
for _, mode in ipairs({ "animation", "muscles" }) do
    restored = nil
    -- (the animation on the model's own body; the muscles' - which can fail - on Homer's, which gets up from the front)
    local b = knocked(mode == "animation" and -10 or -6, mode == "animation" and prof.key or "")
    local cc = { id = b, ent = rag, profile = prof, root = prof.root, npc = { class = "npc_test", health = 50 } }
    local began = D.GetUp(cc, mode)
    local low, ticks = math.huge, 0
    local head = rag:GetPhysicsObjectNum(prof.byName[DM.B .. "Head1"].index)
    while ticks < 600 and not restored do
        delirium.Step(1 / 60)
        ADVANCE(1 / 60)
        D.RiseTick(cc, delirium.State(b))
        low = math.min(low, head:GetPos().z)
        ticks = ticks + 1
        if DEBUG_RISE and ticks % 30 == 0 then
            local st = delirium.State(b)
            OUT(string.format("  %s t %.1f getUp %s standing %s progress %.2f pelvis %.2f rising %s", mode, ticks / 60, tostring(st.getUp), tostring(st.getUpStanding), st.getUpProgress or -1, delirium.Parts(b)[3], tostring(cc.rising ~= nil)))
        end
    end
    if mode == "animation" then
        local top = head:GetPos().z
        check("an NPC's get-up (animation): the ragdoll moved along it, from the floor to its feet", began and low < 25 and top > 50,
              string.format("head from %.0f to %.0f units", low, top))
        check("... the NPC back as it ends, its last pose held meanwhile (no waiting to be still)", restored ~= nil and restored.held,
              restored and string.format("after %.2f s", ticks / 60) or "never")
    else
        local st = delirium.State(b)
        check("an NPC's get-up (muscles): the NPC back once it's on its feet, the get-up still steadying it",
              began and restored ~= nil and st.getUp == "muscles", restored and string.format("after %.2f s", ticks / 60) or "never")
    end
    D.StopGetUp(cc)
end
D.RestoreNpc = realRestore

-- falling: in the air, the high fall; balance gone, the fall caught (the stand-in ragdoll on the stand-in's floor)
do
    local fb = delirium.Create(prof.key, 10, 10, 0, 0, 0.3)
    local fc = { id = fb, ent = rag, profile = prof, root = prof.root }
    local called = {}
    local realHigh, realCatch = delirium.HighFall, delirium.CatchFall
    delirium.HighFall = function(i, set) called.high = (called.high or 0) + 1 return realHigh(i, set) end
    delirium.CatchFall = function(i, set) called.catch = (called.catch or 0) + 1 return realCatch(i, set) end
    local pelvis = rag:GetPhysicsObjectNum(prof.root)
    local was = pelvis:GetPos()
    -- standing, balancing, on the floor: nothing
    pelvis:SetPos(Vector(was.x, was.y, 38))
    pelvis:SetVelocity(Vector(0, 0, 0))
    D.FallTick(fc, { balancing = true })
    ADVANCE(0.2)
    D.FallTick(fc, { balancing = true })
    check("standing on the floor: no fall", not called.high and not called.catch)
    -- its balance gone, going down: the fall caught
    D.FallTick(fc, { balancing = false, catching = false })
    check("its balance gone: it catches the fall", called.catch == 1, called.catch)
    -- high in the air: the high fall (once)
    pelvis:SetPos(Vector(was.x, was.y, 400))
    pelvis:SetVelocity(Vector(0, 0, -200))
    D.FallTick(fc, { balancing = false })
    ADVANCE(0.2)
    D.FallTick(fc, { balancing = false })
    ADVANCE(0.2)
    D.FallTick(fc, { balancing = false })
    check("in the air: the high fall, once", called.high == 1, called.high)
    -- landed: over
    pelvis:SetPos(Vector(was.x, was.y, 8))
    pelvis:SetVelocity(Vector(0, 0, 0))
    for i = 1, 6 do D.FallTick(fc, { balancing = false }) ADVANCE(0.2) end
    check("... over once it has landed", fc.fall and fc.fall.mode == nil, fc.fall and tostring(fc.fall.mode))
    delirium.HighFall, delirium.CatchFall = realHigh, realCatch
    pelvis:SetPos(was)
end

-- ledges, low walls, shocks: the core's side (where the lip is, the hands held on it, letting go; what's a low wall)
-- in a world of boxes - the floor, a platform 100 units high (its wall at x = 0, the platform at x < 0), and a railing
-- 40 high at x 200..206
do
    local M = DM.METRES
    local BOXES = {
        { Vector(-4000, -4000, -50), Vector(4000, 4000, 0) },
        { Vector(-400, -400, 0), Vector(0, 400, 100) },
        { Vector(200, -400, 0), Vector(206, 400, 40) },
    }
    local function boxHit(a, b, lo, hi)
        if a.x > lo.x and a.x < hi.x and a.y > lo.y and a.y < hi.y and a.z > lo.z and a.z < hi.z then return 0, Vector(0, 0, 1), true end
        local t0, t1, n = 0, 1, nil
        for _, ax in ipairs({ "x", "y", "z" }) do
            local d = b[ax] - a[ax]
            if math.abs(d) < 1e-9 then
                if a[ax] < lo[ax] or a[ax] > hi[ax] then return nil end
            else
                local ta, tb = (lo[ax] - a[ax]) / d, (hi[ax] - a[ax]) / d
                local na, nb = Vector(0, 0, 0), Vector(0, 0, 0)
                na[ax], nb[ax] = -1, 1
                if ta > tb then ta, tb, na, nb = tb, ta, nb, na end
                if ta > t0 then t0, n = ta, na end
                if tb < t1 then t1 = tb end
                if t0 > t1 then return nil end
            end
        end
        if not n then return nil end
        return t0, n, false
    end
    local realTrace = util.TraceLine
    util.TraceLine = function(t)
        TRACES = TRACES + 1
        local best
        for _, bx in ipairs(BOXES) do
            local f, n, solid = boxHit(t.start, t.endpos, bx[1], bx[2])
            if f and (not best or f < best.f) then best = { f = f, n = n, solid = solid } end
        end
        if not best then return { Hit = false, HitPos = Vector(t.endpos), Fraction = 1, Entity = NULL } end
        return { Hit = true, StartSolid = best.solid, HitPos = t.start + (t.endpos - t.start) * best.f, HitNormal = best.n,
                 Fraction = best.f, Entity = WORLD, HitWorld = true, PhysicsBone = 0 }
    end
    local spy = {}
    local realMsg = delirium.Message
    delirium.Message = function(i, name, params) spy[#spy + 1] = { name = name, params = params or {} } return realMsg(i, name, params) end
    local function sent(name) for i = #spy, 1, -1 do if spy[i].name == name then return spy[i].params end end end
    local function count(name) local n = 0 for _, m in ipairs(spy) do if m.name == name then n = n + 1 end end return n end

    local lb = delirium.Create(prof.key, 10, 10, 0, 0, 0.3)
    local lrag = ents.Create("prop_ragdoll")
    lrag:SetModel(MODEL)
    -- (just off the platform's wall, the shoulders 15 units under its top, falling)
    local function hang(npc)
        lrag:SetPos(Vector(12, 0, 0))
        lrag:Spawn()
        local la, ra = lrag:GetPhysicsObjectNum(prof.byName[DM.B .. "L_UpperArm"].index), lrag:GetPhysicsObjectNum(prof.byName[DM.B .. "R_UpperArm"].index)
        local s0 = (la:GetPos().z + ra:GetPos().z) / 2
        lrag:SetPos(Vector(12, 0, 85 - s0))
        lrag:Spawn()
        for i = 0, 14 do lrag:GetPhysicsObjectNum(i):SetVelocity(Vector(0, 0, -60)) end
        return { id = lb, ent = lrag, profile = prof, root = prof.root, npc = npc and { class = "npc_test" } or nil, lastHit = 0 }
    end
    local lc = hang(true)
    local lhIndex = prof.byName[DM.B .. "L_Hand"].index
    local pz = lrag:GetPhysicsObjectNum(prof.root):GetPos().z
    D.FallTick(lc, { balancing = false })
    local g = sent("grab")
    check("an NPC's body over a drop, a ledge beside it: it grabs the lip (GTA IV's grab, along it)",
          lc.fall and lc.fall.mode == "ledge" and g and g.linegrab and g.dontLetGo and math.abs(g.pos1[3] - 100 * M) < 0.01
          and math.abs(g.pos1[1]) < 0.02 and math.abs(g.pos2[1]) < 0.02 and sent("highFall") and sent("highFall").start == false,
          g and string.format("pos1 %.2f %.2f %.2f (pelvis %.0f units up)", g.pos1[1], g.pos1[2], g.pos1[3], pz) or "no grab")
    -- its left hand gets there: held on (welded to the world), the fingers curled round it, the legs kicking
    lrag:GetPhysicsObjectNum(lhIndex):SetPos(Vector(1, 4, 97))
    ADVANCE(0.2)
    D.FallTick(lc, { balancing = false, grabbing = true })
    local w = WELDS[#WELDS]
    check("... a hand on the lip: held there (welded to the world), the legs kicking",
          #WELDS == 1 and w.ent1 == lrag and w.ent2 == WORLD and w.bone1 == lhIndex
          and sent("pedalLegs") and sent("pedalLegs").start == true and lc.fall.ledge and lc.fall.ledge.hung ~= nil,
          string.format("%d welds", #WELDS))
    ADVANCE(0.2)
    D.FallTick(lc, { balancing = false, grabbing = true })
    check("... hanging on", lc.fall.ledge ~= nil and w:IsValid() and #WELDS == 1)
    -- hit: it lets go, and falls on (not grabbing at it again on the way down)
    lc.lastHit = CurTime() + 0.01
    ADVANCE(0.1)
    D.FallTick(lc, { balancing = false, grabbing = true })
    check("hit while hanging: it lets go (the weld gone, its grab stopped, the legs still)",
          lc.fall.ledge == nil and not w:IsValid() and sent("grab").start == false and sent("pedalLegs").start == false)
    spy = {}
    D.FallTick(lc, { balancing = false })
    check("... and doesn't grab at it again on the way down", count("grab") == 0)
    -- missed: no hand on it in time, it gives up and falls on
    local mc = hang(true)
    D.FallTick(mc, { balancing = false })
    ADVANCE(1.3)
    D.FallTick(mc, { balancing = false, grabbing = true })
    check("a ledge grabbed at and missed (no hand on it in time): it gives up, falling on", mc.fall.ledge == nil and sent("grab").start == false and #WELDS == 1)
    -- one-handed (the other holding a gun): not for long
    local oc = hang(true)
    D.FallTick(oc, { balancing = false })
    lrag:GetPhysicsObjectNum(lhIndex):SetPos(Vector(1, 4, 97))
    ADVANCE(0.1)
    D.FallTick(oc, { balancing = false, grabbing = true })
    ADVANCE(0.5)
    D.FallTick(oc, { balancing = false, grabbing = true })
    local untilOne = oc.fall.ledge and oc.fall.ledge.untilT
    check("hanging by one hand: it lets go within 3 s", untilOne ~= nil and untilOne - CurTime() <= 3, untilOne and string.format("%.1f s", untilOne - CurTime()))
    ADVANCE(3.1)
    D.FallTick(oc, { balancing = false, grabbing = true })
    check("... and does", oc.fall.ledge == nil and not WELDS[#WELDS]:IsValid())
    -- a player's body doesn't (they grab ledges themselves)
    spy = {}
    local pc = hang(false)
    D.FallTick(pc, { balancing = false })
    check("a player's body (not an NPC's) doesn't grab ledges: they do that themselves", count("grab") == 0)
    -- another addon says no
    hook.Add("DeliriumMayGrab", "test", function() return false end)
    local hc = hang(true)
    D.FallTick(hc, { balancing = false })
    check("DeliriumMayGrab -> false: no grab", count("grab") == 0)
    hook.Remove("DeliriumMayGrab", "test")

    -- a limp arm (cuffed, broken, cut off) doesn't reach for it; both limp, no grab at all
    spy = {}
    local ac = hang(true)
    ac.limpArms = { true, false }
    D.FallTick(ac, { balancing = false })
    local ga = sent("grab")
    check("one arm limp (cuffed, broken): only the other reaches for the lip", ga and ga.useLeft == false and ga.useRight == true)
    lrag:GetPhysicsObjectNum(lhIndex):SetPos(Vector(1, 4, 97))
    ADVANCE(0.1)
    D.FallTick(ac, { balancing = false, grabbing = true, gripLeft = true })
    check("... the limp hand isn't held on, even at the lip", ac.fall.ledge ~= nil and not ac.fall.ledge.held[1])
    D.LetGoLedge(ac, "test")
    spy = {}
    local bc = hang(true)
    bc.limpArms = { true, true }
    D.FallTick(bc, { balancing = false })
    check("both arms limp (cuffed): no grab", count("grab") == 0)

    -- a low wall: knocked back into a railing at hip height, over it (GTA IV's fallOverWall) - then stopped once over
    spy = {}
    local wrag = ents.Create("prop_ragdoll")
    wrag:SetModel(MODEL)
    wrag:SetPos(Vector(186, 0, 0))
    wrag:Spawn()
    for i = 0, 14 do wrag:GetPhysicsObjectNum(i):SetVelocity(Vector(80, 0, 0)) end
    local wc = { id = lb, ent = wrag, profile = prof, root = prof.root, lastHit = 0 }
    local wp = wrag:GetPhysicsObjectNum(prof.root)
    D.FallTick(wc, { balancing = true })
    local fw = sent("fallOverWall")
    check("knocked into a railing at hip height: over it (fallOverWall, at its top edge)",
          fw and fw.start and fw.useSpecifiedPoint and math.abs(fw.fallOverPoint[1] - 200 * M) < 0.01 and math.abs(fw.fallOverPoint[3] - 40 * M) < 0.01
          and math.abs(fw.fallOverEdge[2]) > 0.99 and wc.fall.mode == "wall",
          fw and string.format("at %.2f %.2f %.2f, pelvis %.0f up", fw.fallOverPoint[1], fw.fallOverPoint[2], fw.fallOverPoint[3], wp:GetPos().z) or "none")
    local p = wp:GetPos()
    wp:SetPos(Vector(216, p.y, 50))
    ADVANCE(0.1)
    D.FallTick(wc, { balancing = false })
    check("... the push stopped once it's over the top", sent("fallOverWall").start == false and wc.fall.wallOver)
    -- a wall it can't go over (the platform's, 100 high): no
    spy = {}
    local tall = ents.Create("prop_ragdoll")
    tall:SetModel(MODEL)
    tall:SetPos(Vector(14, 0, 0))
    tall:Spawn()
    for i = 0, 14 do tall:GetPhysicsObjectNum(i):SetVelocity(Vector(-80, 0, 0)) end
    D.FallTick({ id = lb, ent = tall, profile = prof, root = prof.root, lastHit = 0 }, { balancing = true })
    check("knocked into a tall wall: not over it", count("fallOverWall") == 0)
    check("... at its side (the stand-in faces +y): no bracing - the arms don't reach a wall beside or behind", count("dlBrace") == 0)
    -- thrown at the platform's wall ahead of it (its face at y = -400): hands out onto it
    spy = {}
    local ahead = ents.Create("prop_ragdoll")
    ahead:SetModel(MODEL)
    ahead:SetPos(Vector(-200, -414, 0))
    ahead:Spawn()
    for i = 0, 14 do ahead:GetPhysicsObjectNum(i):SetVelocity(Vector(0, 80, 0)) end
    D.FallTick({ id = lb, ent = ahead, profile = prof, root = prof.root, lastHit = 0 }, { balancing = true })
    local br = sent("dlBrace")
    check("thrown at a wall ahead of it: its hands out onto it (dlBrace, on the wall's face), both", br and br.start and math.abs(br.pos[2] + 400 * M) < 0.01 and br.normal[2] < -0.99
          and br.useLeft == true and br.useRight == true,
          br and string.format("at %.2f %.2f %.2f, facing %.2f %.2f", br.pos[1], br.pos[2], br.pos[3], br.normal[1], br.normal[2]) or "none")
    spy = {}
    D.FallTick({ id = lb, ent = ahead, profile = prof, root = prof.root, lastHit = 0 }, { balancing = false })
    check("... lying there sliding at it (not on its feet, not going down from them): no", count("dlBrace") == 0)
    -- walking at it: no
    spy = {}
    local slow = ents.Create("prop_ragdoll")
    slow:SetModel(MODEL)
    slow:SetPos(Vector(14, 0, 0))
    slow:Spawn()
    for i = 0, 14 do slow:GetPhysicsObjectNum(i):SetVelocity(Vector(-30, 0, 0)) end
    D.FallTick({ id = lb, ent = slow, profile = prof, root = prof.root, lastHit = 0 }, { balancing = true })
    check("walking at a wall: no bracing", count("dlBrace") == 0)

    -- shocked: electrocuted, once, for as long as the longest shock lasts
    spy = {}
    local sc = { id = lb, ent = lrag, profile = prof, root = prof.root }
    D.Shock(sc, 1)
    D.Shock(sc, 2.5)
    D.Shock(sc, 0.5)
    check("shocked: electrocuted (once), until the longest shock is over", count("electrocute") == 1 and sent("electrocute").start == true
          and math.abs(sc.shock - (CurTime() + 2.5)) < 1e-6)
    delirium.Message = realMsg
    util.TraceLine = realTrace
end

-- limp arms (cuffed, broken, cut off): their bones let go, with another addon's own
do
    local lc2 = { profile = prof, limpArms = { true, false }, looseBones = { [99] = true } }
    local set = D.LooseSet(lc2)
    local function idx(n) return prof.byName[DM.B .. n].index end
    check("a limp left arm: its bones let go (and another addon's own), the right arm's not",
          set[idx("L_UpperArm")] and set[idx("L_Forearm")] and set[idx("L_Hand")] and not set[idx("R_Hand")] and not set[idx("R_UpperArm")] and set[99])
    check("no limp arms, nothing let go", next(D.LooseSet({ profile = prof })) == nil)
end

-- hit by a car: thrown, it tucks and rolls; knocked slower, it catches the fall; a car barely moving, or a prop, isn't one
do
    local M = DM.METRES
    local spy = {}
    local realMsg, realCatch = delirium.Message, delirium.CatchFall
    delirium.Message = function(i, name, params) spy[#spy + 1] = { name = name, params = params or {} } return realMsg(i, name, params) end
    delirium.CatchFall = function(i, set) spy[#spy + 1] = { name = "CatchFall" } return realCatch(i, set) end
    local function count(name) local n = 0 for _, m in ipairs(spy) do if m.name == name then n = n + 1 end end return n end
    local function car(speed, glide, notVehicle)
        return { IsValid = function() return true end, IsVehicle = function() return not glide and not notVehicle end,
                 IsGlideVehicle = glide or nil, GetVelocity = function() return Vector(speed / M, 0, 0) end, GetParent = function() return NULL end }
    end
    local function hit(inflictor, kind)
        local d = DamageInfo()
        d:SetDamageType(kind)
        d:SetDamage(30)
        d:SetInflictor(inflictor)
        return d
    end
    local cb = delirium.Create(prof.key, 14, 14, 0, 0, 0.3)
    local crag = ents.Create("prop_ragdoll")
    crag:SetModel(MODEL)
    crag:SetPos(Vector(500, 0, 0))
    crag:Spawn()
    local cc = { id = cb, ent = crag, profile = prof, root = prof.root, lastHit = 0 }
    local glide = car(12, true)
    check("a Glide car moving, its crush: a car hit", D.CarHit(hit(glide, DMG_CRUSH)) == glide)
    D.Hurt(cc, crag, hit(glide, DMG_CRUSH))
    check("... thrown (12 m/s): it tucks and rolls (bodyRollUp) a while, then catches itself", count("bodyRollUp") == 1 and cc.carRoll ~= nil
          and math.abs(cc.carRoll - CurTime() - 1.2) < 0.01 and count("CatchFall") == 0, cc.carRoll and string.format("%.2f s", cc.carRoll - CurTime()))
    spy, cc.carRoll = {}, nil
    D.Hurt(cc, crag, hit(car(4), DMG_VEHICLE))
    check("knocked by a slow car (a Source vehicle's hit, 4 m/s): it catches the fall", count("CatchFall") == 1 and count("bodyRollUp") == 0)
    spy = {}
    check("a car barely moving, a thrown prop: not car hits", D.CarHit(hit(car(1), DMG_VEHICLE)) == nil and D.CarHit(hit(car(10, false, true), DMG_CRUSH)) == nil)
    D.Hurt(cc, crag, hit(car(10, false, true), DMG_CRUSH))
    check("... no reaction to a crushing prop", #spy == 0, #spy)
    delirium.Message, delirium.CatchFall = realMsg, realCatch
end

-- a bullet's reaction by where it lands (dlShotReact); a leg shot leaves the leg dead a moment, and the like - the
-- engine's injury, sent from one place with another addon's own lasting hurt
do
    local spy = {}
    local realMsg = delirium.Message
    delirium.Message = function(i, name, params) spy[#spy + 1] = { name = name, params = params or {} } return realMsg(i, name, params) end
    local function sent(name) for i = #spy, 1, -1 do if spy[i].name == name then return spy[i].params end end end
    local function injured(limb) for i = #spy, 1, -1 do if spy[i].name == "injury" and spy[i].params.mask == limb then return spy[i].params.amount end end end
    check("where a bullet lands: the engine's parts to reactions", D.ShotRegion(2) == "leg" and D.ShotRegion(5) == "leg" and D.ShotRegion(8) == "gut"
          and D.ShotRegion(0) == "groin" and D.ShotRegion(10) == "chest" and D.ShotRegion(12) == "head" and D.ShotRegion(18) == "shoulder" and D.ShotRegion(19) == "arm")
    local sb = delirium.Create(prof.key, 18, 18, 0, 0, 0.3)
    local srag = ents.Create("prop_ragdoll")
    srag:SetModel(MODEL)
    srag:SetPos(Vector(700, 0, 0))
    srag:Spawn()
    local sc = { id = sb, ent = srag, profile = prof, root = prof.root, lastHit = 0 }
    local d = DamageInfo()
    d:SetDamageType(DMG_BULLET)
    d:SetDamage(30)
    D.ShotReaction(sc, 2, Vector(-1, 0, 0), d)
    local r = sent("dlShotReact")
    check("shot in the left calf: the leg's reaction, from the left", r and r.start == true and r.region == "leg" and r.side == 0 and math.abs(r.dir[1] + 1) < 1e-6
          and r.power > 0.5 and r.power < 1.5, r and string.format("%s %s power %.2f", r.region, tostring(r.side), r.power) or "none")
    check("... the left leg dead a moment (85%)", math.abs(D.LimbHurt(sc, "ll") - 0.85) < 1e-6 and D.LimbHurt(sc, "lr") == 0)
    D.Injuries(sc)
    check("... sent to the body as its injury", math.abs((injured("ll") or 0) - 0.85) < 1e-6 and injured("lr") == nil)
    sc.limbHurt = { lr = 0.4, ll = 0.3 }
    D.Injuries(sc)
    check("another addon's lasting hurt (a limp): sent too, the worse of the two counting", math.abs((injured("lr") or 0) - 0.4) < 1e-6 and math.abs(injured("ll") - 0.85) < 1e-6)
    ADVANCE(1.7)
    D.Injuries(sc)
    check("... the dead leg over: back to the lasting hurt", math.abs(injured("ll") - 0.3) < 1e-6 and math.abs(D.LimbHurt(sc, "ll") - 0.3) < 1e-6)
    D.ShotReaction(sc, 12, Vector(0, 1, 0), d)
    check("shot in the head, not killed: stunned, the knees going", sent("dlShotReact").region == "head" and D.LimbHurt(sc, "ll") > 0.6 and D.LimbHurt(sc, "lr") > 0.6)
    D.ShotReaction(sc, 18, Vector(0, 1, 0), d)
    check("shot in the right shoulder: spun, that arm dropping", sent("dlShotReact").region == "shoulder" and sent("dlShotReact").side == 1 and D.LimbHurt(sc, "ur") > 0.8 and D.LimbHurt(sc, "ul") == 0)
    -- through D.Hurt: a bullet at the body's left calf
    sc.limbHurt, sc.hurtFor = nil, nil
    local q = delirium.Parts(sb)
    local U = 1 / DM.METRES
    d:SetDamagePosition(Vector(q[2 * 7 + 1] * U, q[2 * 7 + 2] * U, q[2 * 7 + 3] * U))
    d:SetDamageForce(Vector(-300, 0, 0))
    spy = {}
    D.Hurt(sc, srag, d)
    check("... a reaction run (c.lastReact: what's ongoing is sent again after it)", sc.lastReact == CurTime())
    check("a bullet in the calf, as the game hands it on: the leg's reaction, the leg dead", sent("dlShotReact") and sent("dlShotReact").region == "leg" and D.LimbHurt(sc, "ll") > 0.8,
          sent("dlShotReact") and sent("dlShotReact").region or "none")
    -- a shotgun's pellets: bullets too
    local pd = DamageInfo()
    pd:SetDamageType(DMG_BUCKSHOT)
    pd:SetDamage(12)
    pd:SetDamagePosition(d:GetDamagePosition())
    pd:SetDamageForce(Vector(-300, 0, 0))
    spy = {}
    D.Hurt(sc, srag, pd)
    check("a shotgun's pellets (buckshot only): a bullet's reaction too", sent("dlShotReact") ~= nil)
    -- the pelvis and the belly by where the bullet came from
    local fwd, side = D.BodyFacing(sc)
    check("(the body's facing worked out)", fwd ~= nil and side ~= nil)
    if fwd then
        spy = {}
        D.ShotReaction(sc, 0, -fwd, d)
        check("the pelvis from the front: the groin (doubled over)", sent("dlShotReact") and sent("dlShotReact").region == "groin")
        local root = srag:GetPhysicsObjectNum(prof.root):GetPos()
        d:SetDamagePosition(root + side * 5)
        sc.hurtFor = nil
        spy = {}
        D.ShotReaction(sc, 0, fwd, d)
        local hr = sent("dlShotReact")
        check("... from behind, on its left: a hip - that leg goes a moment, no doubling over", hr and hr.region == "leg" and hr.side == 0
              and D.LimbHurt(sc, "ll") > 0.5 and D.LimbHurt(sc, "lr") == 0, hr and (hr.region .. " " .. hr.side) or "none")
        spy = {}
        D.ShotReaction(sc, 8, fwd, d)
        check("the belly from behind: GTA IV's own reaction only (it arches them)", sent("dlShotReact") == nil)
    end
    -- getting up: a hurt limb mostly let go of (else the get-up fails on it)
    sc.hurtFor, sc.limbHurt, sc.hurtSent = nil, nil, nil
    D.Injure(sc, "ll", 0.85, 5)
    sc.rising = true
    spy = {}
    D.Injuries(sc)
    check("getting up with a dead leg: hardly weak while it does (0.2)", math.abs((injured("ll") or 0) - 0.2) < 1e-6, injured("ll"))
    sc.rising = nil
    spy = {}
    D.Injuries(sc)
    check("... and weak again once up", math.abs((injured("ll") or 0) - 0.85) < 1e-6, injured("ll"))
    SET_CVAR("delirium_shot_reactions", "0")
    spy = {}
    D.ShotReaction(sc, 8, Vector(0, 1, 0), d)
    check("delirium_shot_reactions 0: none", sent("dlShotReact") == nil)
    SET_CVAR("delirium_shot_reactions", "1")
    -- just up (holding the get-up's pose, the NPC again in a moment): still hardly weak
    sc.hurtFor, sc.hurtSent = nil, nil
    D.Injure(sc, "lr", 0.85, 5)
    sc.holdPose = { targets = {}, untilT = CurTime() + 0.5 }
    spy = {}
    D.Injuries(sc)
    check("just up, holding the get-up's pose: hardly weak yet (the knee would give before it's done)", math.abs((injured("lr") or 0) - 0.2) < 1e-6, injured("lr"))
    sc.holdPose = nil
    -- an arm another addon has busy (a gun in it): kept strong enough for it
    D.Injure(sc, "ur", 0.85, 5)
    sc.armsBusy = { false, true }
    spy = {}
    D.Injuries(sc)
    check("a hurt arm holding up a gun: kept strong enough for it", math.abs((injured("ur") or 0) - 0.2) < 1e-6, injured("ur"))
    local hands = D.FreeHands(sc)
    check("... and not braced with (the other hand is)", hands and hands[1] == true and hands[2] == false)
    sc.limpArms = { true, false }
    check("the other limp (cuffed, broken): no hand to brace with at all", D.FreeHands(sc) == nil)
    sc.limpArms, sc.armsBusy = nil, nil
    -- every task that stops what the body was doing says so (c.lastReact): the balance, a fall caught, the get-up
    ADVANCE(1)
    local t0 = CurTime()
    D.React(sc, "balance")
    check("balancing afresh (it stops everything): said so - c.lastReact", sc.lastReact == t0)
    ADVANCE(1)
    D.React(sc, "fall")
    check("... a fall caught: the same", sc.lastReact == CurTime())
    -- gone limp (out cold) or killed: the module's own reactions ended, the tone they found not handed back
    spy = {}
    D.React(sc, "relax")
    local er, eb = sent("dlShotReact"), sent("dlBrace")
    check("out cold: its hit's reaction and its brace over, nothing handed back (restore false)", er and er.start == false and er.restore == false
          and eb and eb.start == false and eb.restore == false)
    spy = {}
    sc.dead = nil
    D.Kill(sc, {})
    er = sent("dlShotReact")
    check("killed: the same", er and er.start == false and er.restore == false)
    -- killed: no injuries sent to a dead body
    spy = {}
    sc.dead = CurTime()
    D.Injuries(sc)
    check("dead: its injuries left alone", injured("ll") == nil and injured("lr") == nil)
    delirium.Message = realMsg
end

-- restored, then made a body again in the moment before it's shown: its gun (its other children) stays hidden - not the
-- NPC's own gun in view on the body - and is shown once that body's done
do
    local function stub(kind)
        local e = { _ent = true, kind = kind, nodraw = false, valid = true, flags = 0, solid = 0, move = 2, children = {}, ang = Angle(0, 0, 0) }
        function e:IsValid() return self.valid end
        function e:GetNoDraw() return self.nodraw end
        function e:SetNoDraw(b) self.nodraw = b end
        function e:GetChildren() return self.children end
        function e:GetSolidFlags() return self.solid end
        function e:SetSolidFlags(f) self.solid = f end
        function e:AddSolidFlags(f) self.solid = bit.bor(self.solid, f) end
        function e:GetMoveType() return self.move end
        function e:SetMoveType(m) self.move = m end
        function e:GetFlags() return self.flags end
        function e:AddFlags(f) self.flags = bit.bor(self.flags, f) end
        function e:RemoveFlags(f) self.flags = bit.band(self.flags, bit.bnot(f)) end
        function e:AddEFlags() end function e:RemoveEFlags() end
        function e:SetPos(p) self.pos = p end function e:GetPos() return self.pos or Vector(0, 0, 0) end
        function e:SetAngles(a) self.ang = a end function e:GetAngles() return self.ang end
        function e:SetHealth(h) self.health = h end
        function e:GetClass() return "npc_" .. self.kind end
        return e
    end
    local npc, gun = stub("npc"), stub("weapon")
    npc.children = { gun }
    local function body(x)
        local id = delirium.Create(prof.key, x, 30, 0, 0, 0.3)
        local r = ents.Create("prop_ragdoll")
        r:SetModel(MODEL)
        r:SetPos(Vector(x / DM.METRES, 30 / DM.METRES, 0))
        r:Spawn()
        return { id = id, ent = r, profile = prof, root = prof.root, npc = { class = "npc_test", health = 40 }, replaces = npc, lastHit = 0 }
    end
    local c1 = body(4)
    D.HideNpc(c1)
    check("an NPC made a body: it and its gun hidden", npc.nodraw and gun.nodraw)
    D.RestoreNpc(c1)
    local c2 = body(6)
    D.HideNpc(c2)
    ADVANCE(0.2)
    RUN_TIMERS()
    check("restored, then a body again before it's shown: its gun stays hidden (not the NPC's own in view on the body)", npc.nodraw and gun.nodraw)
    D.RestoreNpc(c2)
    ADVANCE(0.2)
    RUN_TIMERS()
    check("... and shown once that body's done", not npc.nodraw and not gun.nodraw)

    -- shown again while its body has it (the game deploying its gun, another addon equipping it): hidden again at once,
    -- and shown with the rest once the body's done
    local held = stub("weapon")
    function npc:GetActiveWeapon() return held end
    function npc:GetClass() return "npc_test" end
    function held:GetClass() return "weapon_test" end
    local c3 = body(8)
    D.HideNpc(c3)
    gun.nodraw, held.nodraw, npc.nodraw = false, false, false
    D.FollowNpc(c3)
    check("shown again while its body has it - the NPC, its gun, its held weapon: hidden again", npc.nodraw and gun.nodraw and held.nodraw)
    D.RestoreNpc(c3)
    ADVANCE(0.2)
    RUN_TIMERS()
    check("... and all shown once the body's done", not npc.nodraw and not gun.nodraw and not held.nodraw)
    npc.GetActiveWeapon = nil
end

OUT(failed == 0 and "all passed" or (failed .. " FAILED"))
