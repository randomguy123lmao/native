-- Loads the Z-City integration on the stand-in (Z-City itself stubbed): it must load, hook in and survive a tick.
STUB_DM = include("delirium/dl_math.lua")
local failed = 0
local function check(name, ok, detail)
    if not ok then failed = failed + 1 end
    OUT((ok and "PASS  " or "FAIL  ") .. name .. (detail and ("   [" .. tostring(detail) .. "]") or ""))
end
local PHYSMETA = {}
function PHYSMETA:ComputeShadowControl() end
function PHYSMETA:ApplyForceCenter() end
function PHYSMETA:ApplyForceOffset() end
FindMetaTable = function(name) if name == "PhysObj" then return PHYSMETA end return {} end
HOOK_MONITOR_HIGH, HOOK_HIGH = -2, -1
FCVAR_ARCHIVE = 128
engine = engine or { TickCount = function() return 1 end, TickInterval = function() return 1 / 66 end }
ents.FindByClass = function() return {} end
player.GetAll = function() return {} end
hg = { organism = { fake_spine3 = 1 }, Fake = function() end, RagdollCombatInUse = function() return false end }
include("delirium/sv_delirium.lua")
INCLUDE_ADDON = "delirium_zcity"
local ok, err = pcall(include, "delirium_zcity/sv_zcity.lua")
check("sv_zcity.lua loads", ok, err)
ok, err = pcall(include, "delirium_zcity/sv_guns.lua")
check("sv_guns.lua loads", ok, err)
local D = Delirium
check("the Z-City part is there", D.ZCity and D.ZCity.Conditions and D.BulletMomentum)
check("the PhysObj methods wrapped (Z-City's pulls and pushes seen)", PHYSMETA.ComputeShadowControl ~= D.ZCity.raw.shadow)
check("its hooks in", hook.t.Fake and hook.t.Fake.DeliriumZCity and hook.t.Tick.DeliriumZCity and hook.t.Tick.DeliriumZCityGuns)
ok, err = pcall(function() hook.t.Tick.DeliriumZCity() hook.t.Tick.DeliriumZCityGuns() end)
check("a tick with nothing going on", ok, err)
-- the hit momentum from Z-City's own ammunition
hg.ammotypeshuy = { ["5.56x45 mm"] = { BulletSettings = { Mass = 3.56, Speed = 980 } } }
local wep = { IsValid = function() return true end, IsWeapon = function() return true end, Primary = { Ammo = "5.56x45 mm" } }
local d = DamageInfo()
d:SetDamageType(DMG_BULLET)
d:SetDamage(49)
d:SetInflictor(wep)
local j = D.BulletMomentum(d)
check("a 5.56 round's real push: its mass times its speed", j and math.abs(j - 3.49) < 0.01, j)
check("... the core takes it for the hit", D.HitMomentum(d) == j)
d:SetInflictor(NULL)
check("without Z-City's ammunition, by the damage", D.HitMomentum(d) > 2 and D.HitMomentum(d) < 9, D.HitMomentum(d))
-- cut arteries held with GTA IV's own holds: standing, the shot's reach (a body of the module's own, the wound on its
-- right forearm as Z-City records one); lying hurt, injuredOnGround's injuries
D.noHostCheck, D.retryAt, D.failed = true, nil, nil
check("Delirium started", D.Start(), D.failed)
delirium.MeshAdd(1, {-30, -30, 0, 30, -30, 0, 30, 30, 0, -30, 30, 0}, {0, 1, 2, 0, 2, 3}, 0)
local id = delirium.Create("", 0, 0, 0, 0, 0.3)
delirium.Message(id, "bodyBalance", { start = true })
for i = 1, 60 do delirium.Step(1 / 60) end
local U = 1 / 0.0254
local function partAt(k) local p = delirium.Parts(id) return Vector(p[k * 7 + 1] * U, p[k * 7 + 2] * U, p[k * 7 + 3] * U) end
local rag = { IsValid = function() return true end, IsOnFire = function() return false end,
              LookupBone = function(_, name) return isstring(name) and string.StartWith(name, "ValveBiped.") and 7 or nil end,
              GetBonePosition = function(_, b) return partAt(19), Angle(0, 0, 0) end, GetPhysicsObjectNum = function() return nil end }
local c = { id = id, ent = rag, profile = {}, lastHit = 0 }
local org = { arterialwounds = { { 6, Vector(0, 0, 1.5), Angle(0, 0, 0), "ValveBiped.Bip01_R_Forearm", 50, Vector(0, 0, -100), "rarmartery" } } }
c.state = delirium.State(id)
ok, err = pcall(D.ZCity.Conditions, c, org)
check("a cut artery, standing: no error", ok, err)
for i = 1, 30 do delirium.Step(1 / 60) end
c.state = delirium.State(id)
local function clutching() for _, n in ipairs(c.state.extras or {}) do if n == "clutchWound" then return true end end return false end
check("... held (the engine's clutchWound)", c.zcClutchKey == tostring(org.arterialwounds[1]) and clutching(), tostring(c.zcClutchKey) .. " " .. table.concat(c.state.extras or {}, ","))
-- (cleared by something else - a hit's reaction: held again)
delirium.Message(id, "stopAllBehaviours", {})
c.state = delirium.State(id)
pcall(D.ZCity.Conditions, c, org)
for i = 1, 6 do delirium.Step(1 / 60) end
c.state = delirium.State(id)
check("... held again after something clears it", clutching(), table.concat(c.state.extras or {}, ","))
org.arterialwounds = {}
ok, err = pcall(D.ZCity.Conditions, c, org)
for i = 1, 6 do delirium.Step(1 / 60) end
c.state = delirium.State(id)
check("... let go of once it stops bleeding", ok and c.zcClutchKey == nil and not clutching(), err)
org.arterialwounds = { { 6, Vector(0, 0, 1.5), Angle(0, 0, 0), "ValveBiped.Bip01_R_Forearm", 60, Vector(0, 0, -100), "rarmartery" } }
c.zcGround = true
local sent
local real = delirium.Message
delirium.Message = function(i, name, params) if name == "clutchWound" then sent = params end return real(i, name, params) end
ok, err = pcall(D.ZCity.Conditions, c, org)
delirium.Message = real
check("lying hurt: held too", ok and sent and sent.numInjuries == 1 and sent.injury1Component == 19,
      ok and sent and (tostring(sent.numInjuries) .. " " .. tostring(sent.injury1Component)) or tostring(err))
-- an NPC body's Z-City gun shot: its sound and flash are made on the clients, from the muzzle in the body's hand
check("the shot message registered for the clients", NETSTRINGS and NETSTRINGS.DeliriumZCityShot)
SERVER, CLIENT = false, true
local CL_TIMERS, realTimerCreate = {}, timer.Create
timer.Create = function(name, delay, reps, fn) CL_TIMERS[name] = fn end
ok, err = pcall(include, "autorun/client/cl_delirium_zcity.lua")
timer.Create = realTimerCreate
SERVER, CLIENT = true, false
check("the client part loads", ok and NETRECV.DeliriumZCityShot ~= nil, err)
local emitted, shot = false, false
local gun = { IsValid = function() return true end, EntIndex = function() return 5 end }
gun.EmitShoot = function() emitted = true end
gun.Shoot = function() shot = true end
if NETRECV.DeliriumZCityShot then
    NET_DELIVER({ name = "DeliriumZCityShot", values = { gun, Vector(10, 20, 30), Angle(0, 90, 0) } })
end
check("... told of a shot, Z-City's own gunshot sound plays there", emitted)
check("... and only that (Z-City's whole shot, for a hidden NPC's gun, breaks its model)", not shot)
-- the gun in an NPC body's hand (client): drawn from a model of the body's own - not the weapon's, which Z-City removes
-- whenever the weapon goes out of sight (and turns into the plain world model); and from the gun's class where the
-- hidden weapon isn't sent at all
do
    local made, drawn = {}, {}
    local function stubModel(path)
        local m = { path = path, valid = true }
        function m:IsValid() return self.valid end
        function m:Remove() self.valid = false end
        function m:SetNoDraw() end function m:SetModelScale() end function m:SetBodyGroups() end function m:SetBodygroup() end
        function m:LookupSequence() return 0 end function m:ResetSequence() end
        function m:SetRenderOrigin(p) self.pos = p end function m:SetRenderAngles() end function m:SetPos() end function m:SetAngles() end
        function m:SetupBones() end
        function m:DrawModel() drawn[#drawn + 1] = self end
        made[#made + 1] = m
        return m
    end
    ClientsideModel = stubModel
    local function base(t)
        -- (weapons.Get: the gun's table with its base's - homigrad_base's own functions in it)
        t.WorldModel_Transform = function() end
        t.CreateWorldModel = t.CreateWorldModel or function() error("not on a table") end
        t.ShouldUseFakeModel = function(self) return self.WorldModelFake end
        t.WorldPos, t.WorldAng = Vector(1, 0, 0), Angle(0, 0, 0)
        return t
    end
    weapons = { Get = function(class) if class == "weapon_test_gun" then return base({ WorldModelFake = "models/test/c_gun.mdl", WorldModel = "models/test/w_gun.mdl" }) end end }
    local wep = base({ WorldModelFake = "models/test/c_gun.mdl", WorldModel = "models/test/w_gun.mdl" })
    wep.IsValid = function() return true end
    wep.GetClass = function() return "weapon_test_gun" end
    wep.CreateWorldModel = function(self) self.worldModel = stubModel(self.WorldModelFake) return self.worldModel end
    -- (its own drawing, a copy of its own - as GMod gives a weapon its SWEP table's functions)
    local ownDraws = 0
    local function ownDraw() ownDraws = ownDraws + 1 end
    wep.DrawWorldModel, wep.DrawWorldModelTranslucent = ownDraw, ownDraw
    local nw = { gun = wep, class = "weapon_test_gun" }
    local crag = { IsValid = function() return true end, IsDormant = function() return false end, LookupBone = function() return 1 end,
                   GetNWEntity = function() return nw.gun end, GetNWString = function() return nw.class end,
                   GetBoneMatrix = function() return { GetTranslation = function() return Vector(5, 5, 5) end, GetAngles = function() return Angle(0, 0, 0) end } end }
    local realFind = ents.FindByClass
    ents.FindByClass = function(class) return class == "prop_ragdoll" and { crag } or {} end
    local draw = hook.t.PostDrawOpaqueRenderables and hook.t.PostDrawOpaqueRenderables.DeliriumZCityGuns
    local scan = CL_TIMERS.DeliriumZCityGuns
    local cok, cerr = pcall(function()
        scan()
        draw(false, false)
    end)
    local mine = drawn[#drawn]
    check("the gun in the body's hand: drawn from a model of its own (Z-City's own make of it), not the weapon's",
          cok and mine ~= nil and mine ~= wep.worldModel and mine.path == "models/test/c_gun.mdl", cerr)
    pcall(wep.DrawWorldModel, wep)
    pcall(wep.DrawWorldModelTranslucent, wep)
    check("... the weapon's own drawing off while the body holds it (not its plain model as well)", ownDraws == 0, ownDraws)
    -- Z-City removes the weapon's own model (the weapon out of sight): the body's gun is still there
    if IsValid(wep.worldModel) then wep.worldModel:Remove() end
    drawn = {}
    pcall(draw, false, false)
    check("... still drawn when Z-City removes the weapon's own model", drawn[1] == mine)
    -- the hidden weapon not sent here: drawn from the gun's class
    nw.gun = NULL
    drawn = {}
    pcall(function() scan() draw(false, false) end)
    check("... and from the gun's class where the hidden weapon isn't sent here", drawn[1] ~= nil and drawn[1] ~= mine and drawn[1].path == "models/test/c_gun.mdl"
          and not mine:IsValid())
    -- dropped (no gun marked): its model goes
    local last = drawn[1]
    nw.class = ""
    drawn = {}
    pcall(function() scan() draw(false, false) end)
    check("... gone with the gun (its model removed)", #drawn == 0 and not last:IsValid())
    check("... and the weapon's own drawing back (the NPC up again, its gun in its hand)", wep.DrawWorldModel == ownDraw and wep.DrawWorldModelTranslucent == ownDraw)
    ents.FindByClass = realFind
    ClientsideModel, weapons = nil, nil
end
-- an ordinary bleeding wound held too; and Z-City stamping its time each time it bleeds doesn't set the hold off again
c.zcGround, c.zcClutchKey, c.lastHit = nil, nil, 0
org.arterialwounds = {}
local bleeding = { 2.5, Vector(0, 3, 0), Angle(0, 0, 0), "ValveBiped.Bip01_Spine2", 70 }
org.wounds = { bleeding, { 0.1, Vector(0, 0, 0), Angle(0, 0, 0), "ValveBiped.Bip01_L_Thigh", 70 } }
local starts = 0
local real2 = delirium.Message
delirium.Message = function(i, name, params) if name == "clutchWound" and params.start then starts = starts + 1 end return real2(i, name, params) end
for k = 1, 10 do
    bleeding[5] = 70 + k
    c.state = delirium.State(id)
    pcall(D.ZCity.Conditions, c, org)
    for i = 1, 6 do delirium.Step(1 / 60) end
end
delirium.Message = real2
check("a bleeding wound held too (not only arteries), the one bleeding hard", c.zcClutchKey == tostring(bleeding), tostring(c.zcClutchKey))
check("... held once: Z-City stamping it as it bleeds doesn't set the hold off again", starts == 1, starts)
org.wounds = {}
check("a player's ragdoll doesn't get up by Delirium (Z-City's Fake Up has it)", D.GetUp({ id = id, ent = rag, profile = {}, zcPlayer = {} }, "animation") == false)
-- hanging from a ledge: its hands are on the lip, not on its wounds
c.zcClutchKey, c.zcHeldWounds = nil, nil
delirium.Message(id, "stopAllBehaviours", {})
org.wounds = { { 2.5, Vector(0, 3, 0), Angle(0, 0, 0), "ValveBiped.Bip01_Spine2", 70 } }
c.fall = { ledge = {} }
c.state = delirium.State(id)
ok, err = pcall(D.ZCity.Conditions, c, org)
check("hanging from a ledge: no hand on a wound (both on the lip)", ok and c.zcClutchKey == nil, err or tostring(c.zcClutchKey))
c.fall, org.wounds = nil, {}
-- tasered (Z-City's taser): electrocuted (GTA IV's) while it lasts; the taser's own stiffening of the ragdoll (Z-City's
-- shadow control on its bones) left off the body meanwhile
do
    local calls = 0
    hg.ShadowControl = function() calls = calls + 1 end
    hook.t.InitPostEntity.DeliriumZCity()
    local tc = { id = id, ent = rag, profile = {}, lastHit = 0 }
    D.chars[rag] = tc
    local sentE
    local real3 = delirium.Message
    delirium.Message = function(i, name, params) if name == "electrocute" then sentE = params end return real3(i, name, params) end
    tc.state = delirium.State(id)
    local tok, terr = pcall(D.ZCity.Conditions, tc, { tasered = CurTime() + 5 })
    check("tasered: electrocuted, for as long as Z-City has it", tok and sentE and sentE.start == true and tc.shock and tc.shock >= CurTime() + 5 and tc.shock < CurTime() + 5.3,
          terr or tostring(tc.shock))
    hg.ShadowControl(rag, 3, 0.001, Angle(0, 0, 0), 1000, 50, Vector(0, 0, 0), 0, 0)
    check("... the taser's own stiffening left off it meanwhile", calls == 0, calls)
    tc.shock = nil
    hg.ShadowControl(rag, 3, 0.001, Angle(0, 0, 0), 1000, 50, Vector(0, 0, 0), 0, 0)
    check("... and not otherwise", calls == 1, calls)
    -- Z-City's taser timer outlasting its org.tasered (a 0.01 s timer runs once a tick: a fifth longer at 66 a second):
    -- the shock goes on to its last, and its stiffening stays off the body to the last
    local realExists, realReps = timer.Exists, timer.RepsLeft
    rag.EntIndex = function() return 77 end
    local reps = 100
    timer.Exists = function(name) return name == "Tasering" .. rag:EntIndex() and reps > 0 end
    timer.RepsLeft = function() return reps end
    tok, terr = pcall(D.ZCity.Conditions, tc, { tasered = CurTime() - 0.5 })
    local want = CurTime() + 100 * math.max(0.01, engine.TickInterval())
    check("Z-City's taser still at it past its org.tasered: still electrocuted, to the timer's last", tok and tc.shock and tc.shock >= want,
          terr or string.format("shock until +%.2f s, the timer +%.2f s", (tc.shock or 0) - CurTime(), want - CurTime()))
    tc.shock = nil
    calls = 0
    hg.ShadowControl(rag, 3, 0.001, Angle(0, 0, 0), 1000, 50, Vector(0, 0, 0), 0, 0)
    check("... and its stiffening kept off the body to the timer's last (even between the shock's own checks)", calls == 0, calls)
    reps = 0
    hg.ShadowControl(rag, 3, 0.001, Angle(0, 0, 0), 1000, 50, Vector(0, 0, 0), 0, 0)
    check("... the timer done: let through again", calls == 1, calls)
    timer.Exists, timer.RepsLeft = realExists, realReps
    rag.EntIndex = nil
    D.chars[rag] = nil
    delirium.Message = real3
    delirium.Message(id, "electrocute", { start = false })
end
-- arms that can't be used: broken, cut off, cuffed - limp, and holding nothing
do
    local sentC
    local real4 = delirium.Message
    delirium.Message = function(i, name, params) if name == "clutchWound" and params.start then sentC = params end return real4(i, name, params) end
    delirium.Message(id, "stopAllBehaviours", {})
    local ac = { id = id, ent = rag, profile = {}, lastHit = 0 }
    local aorg = { larm = 1, wounds = { { 2.5, Vector(0, 3, 0), Angle(0, 0, 0), "ValveBiped.Bip01_Spine2", 70 } } }
    ac.state = delirium.State(id)
    local aok, aerr = pcall(D.ZCity.Conditions, ac, aorg)
    check("a broken left arm: limp, and the wound held with the right only", aok and ac.limpArms and ac.limpArms[1] and not ac.limpArms[2]
          and sentC and sentC.dontReachWithLeft == true and sentC.dontReachWithRight == false, aerr)
    aorg.larm, aorg.rarmamputated = 0.5, true
    pcall(D.ZCity.Conditions, ac, aorg)
    check("... healing (Z-City's under 1), it's back; the right cut off: that one's limp", not ac.limpArms[1] and ac.limpArms[2])
    rag.handcuffed = true
    sentC = nil
    ac.zcClutchKey, ac.zcHeldWounds = nil, nil
    delirium.Message(id, "stopAllBehaviours", {})
    ac.state = delirium.State(id)
    pcall(D.ZCity.Conditions, ac, aorg)
    check("cuffed: both arms limp, no wound held", ac.limpArms[1] and ac.limpArms[2] and sentC == nil and ac.zcClutchKey == nil)
    rag.handcuffed = nil
    delirium.Message = real4
end
-- choked (another's ragdoll's hands welded on its neck): its hands go to theirs, its legs thrash; let go, over
do
    local sentC, writhe
    local real5 = delirium.Message
    delirium.Message = function(i, name, params)
        if name == "clutchWound" and params.start then sentC = params end
        if name == "bodyWrithe" then writhe = params end
        return real5(i, name, params)
    end
    delirium.Message(id, "stopAllBehaviours", {})
    local neck = partAt(11)
    local strangler = { IsValid = function() return true end,
                        LookupBone = function(_, name) return (name == "ValveBiped.Bip01_L_Hand" and 1) or (name == "ValveBiped.Bip01_R_Hand" and 2) or nil end,
                        GetBonePosition = function(_, b) return neck + Vector(4, b == 1 and 2 or -2, 0), Angle(0, 0, 0) end }
    local cc = { id = id, ent = rag, profile = {}, lastHit = 0 }
    cc.state = { balancing = false, extras = {}, behaviours = {} }
    D.ZCity.chokedBy = { [rag] = strangler }
    local cok, cerr = pcall(D.ZCity.Conditions, cc, {})
    check("choked: its hands go to the hands at its throat (two, held as wounds), its legs thrash",
          cok and cc.zcChoke ~= nil and sentC and sentC.numInjuries == 2 and writhe and writhe.start and writhe.mask == "lb", cerr)
    D.ZCity.chokedBy = {}
    sentC = nil
    pcall(D.ZCity.Conditions, cc, {})
    check("... let go of: over (its legs still, its hands off)", cc.zcChoke == nil and writhe.start == false and cc.zcClutchKey == nil)
    D.ZCity.chokedBy = { [rag] = strangler }
    cc.unconscious = true
    writhe = nil
    pcall(D.ZCity.Conditions, cc, {})
    check("out cold, it doesn't fight it", cc.zcChoke == nil and writhe == nil)
    D.ZCity.chokedBy = nil
    delirium.Message = real5
end
-- in severe pain, lying there awake: curled up (bodyFoetal), over the writhing; not resting or getting up meanwhile;
-- curled again after a hit has played out; something else (the fire) has it, or the pain eases: uncurled, writhing again
do
    local sent = {}
    local real6 = delirium.Message
    delirium.Message = function(i, name, params) sent[#sent + 1] = { name = name, params = params or {} } return real6(i, name, params) end
    local function last(name) for k = #sent, 1, -1 do if sent[k].name == name then return sent[k].params end end end
    local function count(name) local n = 0 for _, m in ipairs(sent) do if m.name == name then n = n + 1 end end return n end
    delirium.Message(id, "stopAllBehaviours", {})
    local pc = { id = id, ent = rag, profile = { byName = {} }, lastHit = 0, zcGround = true }
    pc.state = { balancing = false, extras = { "injuredOnGround" }, behaviours = {} }
    local porg = { pain = 70 }
    local pok, perr = pcall(D.ZCity.Conditions, pc, porg)
    check("in severe pain (70), lying awake: curled up, the writhing stopped for it", pok and pc.zcCurl ~= nil and last("bodyFoetal") and last("bodyFoetal").start == true
          and last("injuredOnGround") and last("injuredOnGround").start == false, perr)
    check("... not resting, not getting up meanwhile", hook.Run("DeliriumMayRest", pc) == false and hook.Run("DeliriumMayGetUp", pc) == false)
    porg.pain = 50
    pcall(D.ZCity.Conditions, pc, porg)
    check("... still curled at 50 (until it's under 40)", pc.zcCurl ~= nil)
    sent = {}
    ADVANCE(0.2)
    pc.lastHit, pc.lastReact = CurTime(), CurTime()
    pcall(D.ZCity.Conditions, pc, porg)
    local during = count("bodyFoetal")
    ADVANCE(0.7)
    pcall(D.ZCity.Conditions, pc, porg)
    check("hit: curled again once the hit has played out (not over its reaction)", during == 0 and count("bodyFoetal") == 1 and last("bodyFoetal").start == true, during)
    sent = {}
    pc.held = true
    pcall(D.ZCity.Conditions, pc, porg)
    check("carried (something else has it): uncurled, and not writhing either", pc.zcCurl == nil and last("bodyFoetal") and last("bodyFoetal").start == false and count("injuredOnGround") == 0)
    pc.held = nil
    porg.pain = 75
    pcall(D.ZCity.Conditions, pc, porg)
    sent = {}
    porg.pain = 30
    pcall(D.ZCity.Conditions, pc, porg)
    check("the pain eased (30): uncurled, writhing again", pc.zcCurl == nil and last("bodyFoetal").start == false and last("injuredOnGround") and last("injuredOnGround").start == true)
    sent = {}
    porg.pain = 90
    pc.state.balancing = true
    pcall(D.ZCity.Conditions, pc, porg)
    check("on its feet: never", pc.zcCurl == nil and count("bodyFoetal") == 0)
    pc.state.balancing, pc.zcPlayer, pc.zcCrawl = false, {}, CurTime()
    pcall(D.ZCity.Conditions, pc, porg)
    check("a player crawling (W): their crawl, not the curl", pc.zcCurl == nil and count("bodyFoetal") == 0)
    delirium.Message = real6
    delirium.Message(id, "stopAllBehaviours", {})
end
-- the PhysObj pushes: a bad one (not a vector - Z-City's recoil push handed a number) skipped and said, not an error;
-- a good one passed on
do
    local logged, passed = nil, nil
    local realLog, realForce = D.log, D.ZCity.raw.force
    D.log = function(fmt, ...) logged = string.format(fmt, ...) end
    D.ZCity.raw.force = function(phys, f) passed = f end
    local fakePhys = { GetEntity = function() return NULL end }
    local bok, berr = pcall(PHYSMETA.ApplyForceCenter, fakePhys, 1234)
    check("a push that isn't a vector: skipped (no error), and said where from", bok and passed == nil and logged ~= nil and string.find(logged, "not a vector") ~= nil, berr or logged)
    local v = Vector(1, 2, 3)
    bok, berr = pcall(PHYSMETA.ApplyForceCenter, fakePhys, v)
    check("... a vector passed on as it was", bok and passed == v, berr)
    D.log, D.ZCity.raw.force = realLog, realForce
end
-- aiming: not while busy with something its arms are part of; stopping, a get-up isn't cut short, a body lying there
-- isn't made to catch a fall, and one on its feet balances again
do
    local occ = D.ZCity.AimOccupied
    check("no aiming while getting up (or holding the pose at its end, or the NPC coming back), burning, shocked, in a high fall, or thrown by a car",
          occ({ rising = true }) and occ({ restoring = true }) and occ({ holdPose = {} }) and occ({ zcBurning = true }) and occ({ shock = 5 })
          and occ({ fall = { mode = "high" } }) and occ({ carRoll = 3 }) and not occ({}) and not occ({ fall = { mode = "teeter" } }))
    -- down on the floor fighting: the get-up waits a few seconds of it, then goes ahead
    local mayUp = hook.t.DeliriumMayGetUp and hook.t.DeliriumMayGetUp.DeliriumZCityGuns
    local fc = { zcGun = { busy = true, floorSince = CurTime(), lastSeen = CurTime(), floorFor = 4 } }
    local during = mayUp and mayUp(fc)
    ADVANCE(4.2)
    fc.zcGun.lastSeen = CurTime()
    local after = mayUp and mayUp(fc)
    local fc2 = { zcGun = { busy = true, floorSince = CurTime(), lastSeen = CurTime() - 2, floorFor = 4 } }
    check("shooting from the floor at someone it sees: the get-up waits; a few seconds on, or nobody in sight, it gets up",
          mayUp ~= nil and during == false and after == nil and mayUp(fc2) == nil and mayUp({}) == nil)
    local reacted, sent = {}, {}
    local realReact, real7 = D.React, delirium.Message
    D.React = function(c, how) reacted[#reacted + 1] = how or "default" end
    delirium.Message = function(i, name, params) sent[#sent + 1] = { name = name, params = params } return real7(i, name, params) end
    local function aimingBody(state, extra)
        local arag = { IsValid = function() return true end, LookupBone = function() return nil end, GetPhysicsObjectNum = function() return nil end,
                       SetNWEntity = function() end, SetNWString = function() end }
        local ac = { id = id, ent = arag, profile = { byName = {} }, lastHit = 0, state = state }
        for k, v in pairs(extra or {}) do ac[k] = v end
        ac.zcGun = { wep = { IsValid = function() return true end }, aiming = true, busy = true, nextShot = 0, geo = {}, burst = 0 }
        D.chars[arag] = ac
        return ac, arag
    end
    local tick = hook.t.Tick.DeliriumZCityGuns
    local rc, rr = aimingBody({ balancing = true }, { rising = true })
    pcall(tick)
    check("getting up while aiming: the aim dropped, the get-up not cut short", rc.zcGun.aiming == nil and #reacted == 0)
    D.chars[rr] = nil
    reacted = {}
    local lc, lr = aimingBody({ balancing = false }, { zcBurning = true })
    pcall(tick)
    check("lying there (on fire): the aim dropped, not made to catch a fall", lc.zcGun.aiming == nil and #reacted == 0)
    D.chars[lr] = nil
    reacted = {}
    local sc2, sr = aimingBody({ balancing = true }, { held = true })
    pcall(tick)
    check("carried off: the aim dropped, the carrying left be (no balancing started)", sc2.zcGun.aiming == nil and #reacted == 0, tostring(reacted[1]))
    D.chars[sr] = nil
    reacted = {}
    local tc2, tr2 = aimingBody({ balancing = true })
    tr2.LookupBone = function() return 1 end
    tr2.GetBoneMatrix = function() return { GetTranslation = function() return Vector(0, 0, 50) end, GetAngles = function() return Angle(0, 0, 0) end } end
    tc2.zcGun.geo = { muzzlePos = Vector(10, 0, 0), muzzleAng = Angle(0, 0, 0) }
    local tok, terr = pcall(tick)
    check("on its feet, its target gone: the aim dropped, balancing again", tok and tc2.zcGun and tc2.zcGun.aiming == nil and reacted[1] == "balance", terr or tostring(reacted[1]))
    D.chars[tr2] = nil
    D.React, delirium.Message = realReact, real7
end
-- a Z-City gun in a body's hand: its muzzle where Z-City itself fires from (its gun frame on the hand, LocalMuzzle) - not
-- the plain model's attachment; and the arm steered so the barrel, set in the hand its own way, points at the target
do
    local turned = D.ZCity.Turned
    local function near(a, b) return (a - b):Length() < 1e-3 end
    -- (the barrel along +y with the arm along +x: a target along +y wants the arm as it is; one along +x, the arm along
    -- -y; the barrel straight back along the arm: turned right round)
    check("turning: the barrel's offset from the arm taken off the wanted way",
          near(turned(Vector(0, 1, 0), Vector(0, 1, 0), Vector(1, 0, 0)), Vector(1, 0, 0)) and near(turned(Vector(1, 0, 0), Vector(0, 1, 0), Vector(1, 0, 0)), Vector(0, -1, 0))
          and near(turned(Vector(1, 0, 0), Vector(1, 0, 0), Vector(-1, 0, 0)), Vector(-1, 0, 0)))
    -- the steering: the arm where it was asked (along +x), the barrel along +y, the target along +x - the ask turned half
    -- way (45 degrees toward -y, which brings the barrel half way round); the arm not there yet - eased toward the
    -- target, not wound on
    local function physAt(p) return { IsValid = function() return true end, GetPos = function() return p end } end
    local sc = { profile = { byName = { ["ValveBiped.Bip01_R_UpperArm"] = { index = 1 }, ["ValveBiped.Bip01_R_Hand"] = { index = 2 } } } }
    sc.ent = { LookupBone = function() return 5 end,
               GetBoneMatrix = function() return { GetTranslation = function() return Vector(20, 0, 50) end, GetAngles = function() return Angle(0, 90, 0) end } end,
               GetPhysicsObjectNum = function(_, i) return (i == 1 and physAt(Vector(0, 0, 50))) or (i == 2 and physAt(Vector(20, 0, 50))) or nil end }
    local sg = { geo = { muzzlePos = Vector(0, 0, 0), muzzleAng = Angle(0, 0, 0) }, armAsk = Vector(1, 0, 0) }
    D.ZCity.Steer(sc, sg, Vector(2000, 0, 50))
    check("steering: the arm where it was asked, the barrel off - the ask turned half the way it's off", near(sg.armAsk, Vector(0.7071, -0.7071, 0)), tostring(sg.armAsk))
    sg.armAsk = Vector(0, 1, 0)
    D.ZCity.Steer(sc, sg, Vector(2000, 0, 50))
    check("... the arm not there yet: eased toward the target, not wound on", sg.armAsk.x > 0.1 and sg.armAsk.x < 0.4 and sg.armAsk.y > 0.9, tostring(sg.armAsk))
    sg.armAsk, sg.lastArm = Vector(1, 0, 0), Vector(0, 1, 0)
    D.ZCity.Steer(sc, sg, Vector(2000, 0, 50))
    check("... the arm still swinging (moved a lot since the last look): not chased - the ask kept", near(sg.armAsk, Vector(1, 0, 0)), tostring(sg.armAsk))
    local zwep = { IsValid = function() return true end, GetClass = function() return "weapon_osipr_test" end, Primary = { Wait = 0.1, Force = 50 },
                   Shoot = function() end, GetTrace = function() end, WorldModel = "models/weapons/w_irifle.mdl",
                   WorldPos = Vector(15, -0.5, -1.5), WorldAng = Angle(0, 180, 0), LocalMuzzlePos = Vector(-9.963, -0.818, 3.582), LocalMuzzleAng = Angle(0.4, 180, 0),
                   GetSkin = function() return 0 end }
    local znpc = { IsValid = function() return true end, GetActiveWeapon = function() return zwep end, GetClass = function() return "npc_test" end }
    local zrag = { IsValid = function() return true end, LookupBone = function() return 7 end, SetNWEntity = function() end, SetNWString = function() end }
    local zc = { id = id, ent = zrag, profile = {}, lastHit = 0 }
    weapons = { GetStored = function() return nil end }
    if hook.t.DeliriumNpcConverted and hook.t.DeliriumNpcConverted.DeliriumZCityGuns then hook.t.DeliriumNpcConverted.DeliriumZCityGuns(zc, znpc) end
    local geo = zc.zcGun and zc.zcGun.geo
    local f = geo and geo.muzzleAng:Forward()
    check("a Z-City gun: its muzzle as Z-City fires from it - 25 units ahead of the hand, pointing along it",
          geo ~= nil and math.abs(geo.muzzlePos.x - 24.96) < 0.1 and f.x > 0.99, geo and string.format("muzzle %s, pointing %s", tostring(geo.muzzlePos), tostring(f)) or "no gun")
    if delirium.HoldsGun then delirium.HoldsGun(id, false, false) end
    weapons = nil
end
-- fentanyl: ragdolled under it (short of air, out from it - not knocked down by a hit), on their feet: the fold at once;
-- passing out drugged on their feet: the fold; folded awake and the drug worn off: unfolded; folded out cold, they
-- don't get up from it (awake, that's Z-City's to say)
do
    local folds = D.ZCity.FoldsOnRagdoll
    check("ragdolled under fentanyl, on their feet: folding over (not a fall); knocked down by a hit, or not drugged, or down already: not",
          folds({ analgesia = 1.3 }, true, nil) and not folds({ analgesia = 1.3 }, true, { time = CurTime() }) and not folds({ analgesia = 0.5 }, true, nil)
          and not folds({ analgesia = 1.3 }, false, nil))
    local sent = {}
    local real8 = delirium.Message
    delirium.Message = function(i, name, params) sent[#sent + 1] = { name = name, params = params or {} } return real8(i, name, params) end
    local function last(name) for k = #sent, 1, -1 do if sent[k].name == name then return sent[k].params end end end
    delirium.Message(id, "stopAllBehaviours", {})
    local fc = { id = id, ent = rag, profile = { byName = {} }, lastHit = 0, state = { balancing = true, extras = {}, behaviours = {} } }
    local forg = { analgesia = 1.3, otrub = true }
    local fok, ferr = pcall(D.ZCity.Conditions, fc, forg)
    check("passing out drugged on their feet (a light dose too): the fold", fok and fc.zcFold and fc.unconscious and last("dlFentanylFold") and last("dlFentanylFold").start == true, ferr)
    local ply = { IsValid = function() return true end, FakeRagdoll = rag }
    D.chars[rag] = fc
    check("... folded out cold: they don't get up from it", hook.t["Should Fake Up"].DeliriumZCity(ply) == false)
    fc.unconscious = nil
    check("... folded awake (short of air): Z-City's to say", hook.t["Should Fake Up"].DeliriumZCity(ply) == nil)
    forg.otrub, forg.analgesia = false, 0.6
    pcall(D.ZCity.Conditions, fc, forg)
    check("folded awake, the drug worn off: unfolded", fc.zcFold == nil and last("dlFentanylFold").start == false)
    D.chars[rag] = nil
    delirium.Message = real8
    delirium.Message(id, "stopAllBehaviours", {})
end
-- brain damage: its severity followed as Z-City's brain creeps up; posturing held out cold (a coma's), not let go
-- limp; Z-City's fencing response as the arms drawn in; posturing, nothing else takes the arms, and an NPC doesn't
-- get up; no longer posturing out cold: limp
do
    local sent = {}
    local real9 = delirium.Message
    delirium.Message = function(i, name, params) sent[#sent + 1] = { name = name, params = params or {} } return real9(i, name, params) end
    local function last(name) for k = #sent, 1, -1 do if sent[k].name == name then return sent[k].params end end end
    local function count(name, f) local n = 0 for _, m in ipairs(sent) do if m.name == name and (not f or f(m.params)) then n = n + 1 end end return n end
    delirium.Message(id, "stopAllBehaviours", {})
    local bc = { id = id, ent = rag, profile = { byName = {} }, lastHit = 0, state = { balancing = false, extras = {}, behaviours = {} } }
    local borg = { brain = 0.15 }
    for k = 0, 15 do
        borg.brain = 0.15 + k * 0.02
        pcall(D.ZCity.Conditions, bc, borg)
    end
    local sev = last("dlBrainDamage")
    check("brain damage creeping up (0.02 at a time): the body's severity follows it", count("dlBrainDamage") >= 5 and sev and sev.severity and sev.severity > 0.4,
          string.format("%d sent, the last %.2f", count("dlBrainDamage"), sev and sev.severity or -1))
    -- out cold at 0.7: posturing held, not let go limp
    sent = {}
    borg.brain, borg.otrub = 0.7, true
    pcall(D.ZCity.Conditions, bc, borg)
    check("out cold, posturing (0.7): the posture held - not let go limp", bc.unconscious ~= nil and count("bodyRelax", function(q) return q.relaxation == 100 end) == 0
          and bc.zcBrain ~= nil and D.ZCity.Postures(bc, borg))
    -- posturing: no wound held, an NPC doesn't get up
    borg.wounds = { { 2.5, Vector(0, 3, 0), Angle(0, 0, 0), "ValveBiped.Bip01_Spine2", 70 } }
    bc.unconscious, borg.otrub = nil, false
    sent = {}
    pcall(D.ZCity.Conditions, bc, borg)
    bc.zcNpc = { IsValid = function() return true end, organism = borg }
    check("posturing awake: no hand on a wound, and an NPC doesn't get up", count("clutchWound", function(q) return q.start == true end) == 0
          and hook.t.DeliriumMayGetUp.DeliriumZCity(bc) == false)
    bc.zcNpc, borg.wounds = nil, nil
    -- out cold, then no longer posturing: limp
    bc.unconscious, borg.otrub = CurTime(), true
    pcall(D.ZCity.Conditions, bc, borg)
    sent = {}
    borg.brain = 0.3
    pcall(D.ZCity.Conditions, bc, borg)
    check("out cold and no longer posturing: the brain's reaction off, limp", bc.zcBrain == nil and last("dlBrainDamage") and last("dlBrainDamage").start == false
          and count("bodyRelax", function(q) return q.relaxation == 100 end) == 1)
    -- out cold at 0.3 from awake: limp at once
    bc.unconscious, borg.otrub = nil, false
    pcall(D.ZCity.Conditions, bc, borg)
    sent = {}
    borg.otrub = true
    pcall(D.ZCity.Conditions, bc, borg)
    check("out cold, mildly brain damaged: limp, as anyone out cold", bc.unconscious ~= nil and count("bodyRelax", function(q) return q.relaxation == 100 end) == 1 and bc.zcBrain == nil)
    -- the fencing response: the arms drawn in for it, then over
    bc.unconscious, borg.otrub, borg.brain = nil, false, 0
    pcall(D.ZCity.Conditions, bc, borg)
    sent = {}
    borg.fencing, borg.fencingEnd = true, CurTime() + 5
    pcall(D.ZCity.Conditions, bc, borg)
    local fs = last("dlBrainDamage")
    check("Z-City's fencing response (a blow to the head): the arms drawn in (decorticate) for it", fs and fs.start == true and fs.posture == 1)
    borg.fencingEnd = CurTime() - 1
    pcall(D.ZCity.Conditions, bc, borg)
    check("... and over when it is", bc.zcBrain == nil and last("dlBrainDamage").start == false)
    delirium.Message = real9
    delirium.Message(id, "stopAllBehaviours", {})
end
-- a hurt leg (a limp), bleeding out, short of air: on a body standing at rest, its bones where a standing one's are
-- (facing +x, its left +y)
;(function()
    local sent = {}
    local realM = delirium.Message
    delirium.Message = function(i, name, params) sent[#sent + 1] = { name = name, params = params or {} } return realM(i, name, params) end
    local function last(name) for k = #sent, 1, -1 do if sent[k].name == name then return sent[k].params end end end
    local function lastInjury(mask) for k = #sent, 1, -1 do if sent[k].name == "injury" and sent[k].params.mask == mask then return sent[k].params.amount end end end
    local BONES = { Pelvis = Vector(0, 0, 40), Spine2 = Vector(0, 0, 50), Neck1 = Vector(0, 0, 62), Head1 = Vector(0, 0, 66),
                    L_Thigh = Vector(0, 4, 38), R_Thigh = Vector(0, -4, 38), L_UpperArm = Vector(0, 7, 58), R_UpperArm = Vector(0, -7, 58) }
    local names, ids, n = {}, {}, 0
    for k in pairs(BONES) do n = n + 1 names[n] = k ids["ValveBiped.Bip01_" .. k] = n end
    local brag = { IsValid = function() return true end, IsOnFire = function() return false end,
                   LookupBone = function(_, name) return ids[name] end,
                   GetBonePosition = function(_, b) local k = names[b] if not k then return nil end return Vector(BONES[k].x, BONES[k].y, BONES[k].z), Angle(0, 0, 0) end,
                   GetPhysicsObjectNum = function() return nil end }
    delirium.Message(id, "stopAllBehaviours", {})
    local bc = { id = id, ent = brag, profile = { byName = {} }, lastHit = 0 }
    bc.state = { balancing = true, extras = {}, behaviours = {} }
    local org = { lleg = 0.6, rleg = 0, pain = 0, blood = 5000, o2 = { 30 } }
    local function cond() local ok, err = pcall(D.ZCity.Conditions, bc, org) if not ok then OUT("      (error: " .. tostring(err) .. ")") end return ok end
    cond()
    check("a hurt left leg (Z-City's 0.6): weaker, as an injury of about half", bc.limbHurt and math.abs(bc.limbHurt.ll - 0.45) < 0.01 and bc.limbHurt.lr == nil,
          bc.limbHurt and bc.limbHurt.ll)
    local ln = last("leanInDirection")
    check("... on its feet: the weight off it, onto the right leg", ln and ln.leanAmount > 0.05 and ln.dir[2] < -0.9,
          ln and string.format("%.2f towards %.2f %.2f", ln.leanAmount, ln.dir[1], ln.dir[2]) or "no lean")
    D.Injuries(bc)
    check("... the core sends it to the body", math.abs((lastInjury("ll") or 0) - 0.45) < 0.01)
    org.lleg = 0
    sent = {}
    cond()
    check("healed: nothing weak, the lean let go of", bc.limbHurt == nil and last("leanInDirection") and last("leanInDirection").leanAmount == 0)
    org.rlegdislocation = true
    cond()
    check("a dislocated leg: weak much the same", bc.limbHurt and bc.limbHurt.lr == 0.6)
    org.rlegdislocation = nil
    org.larm = 0.7
    cond()
    check("a hurt arm: weaker too (the lean's the legs' only)", bc.limbHurt and bc.limbHurt.ul > 0.3 and bc.limbHurt.ul <= 0.6 and not bc.zcLean)
    org.larm = 0
    -- bleeding out
    org.blood = 3500
    sent = {}
    cond()
    check("bleeding (blood 3500), on its feet: unsteady - a sway", bc.zcBleed == 1 and bc.zcLean and last("leanInDirection") ~= nil and bc.limbHurt == nil)
    check("... it may get up still", hook.Run("DeliriumMayGetUp", bc) ~= false)
    org.blood = 3000
    cond()
    check("bleeding out (3000): the legs going - both weaker - and it stays down", bc.zcBleed == 2 and bc.limbHurt and bc.limbHurt.ll >= 0.45 and bc.limbHurt.lr >= 0.45
          and hook.Run("DeliriumMayGetUp", bc) == false, bc.limbHurt and bc.limbHurt.ll)
    org.blood = 2500
    bc.state.balancing = false
    bc.state.extras = { "injuredOnGround" }
    bc.zcGround = true
    sent = {}
    cond()
    check("bleeding out (2500), lying: slumped - weak all over, the writhing gone out of it", bc.zcSlumped and last("bodyRelax") and last("bodyRelax").relaxation == 45
          and last("injuredOnGround") and last("injuredOnGround").start == false and bc.zcGround == nil)
    check("... the sway let go of, lying", not bc.zcLean)
    sent = {}
    ADVANCE(0.1)
    bc.lastHit, bc.lastReact = CurTime(), CurTime()
    ADVANCE(0.7)
    cond()
    local again = last("bodyRelax")
    org.blood = 5000
    bc.state.extras = {}
    sent = {}
    cond()
    check("slumped and hit: slumped again once the hit has played out", again and again.relaxation == 45)
    check("blood back: firm again (after hits: slumped again once each has played out)", bc.zcSlumped == nil and last("bodyRelax") and last("bodyRelax").relaxation == 0 and bc.zcBleed == nil)
    -- short of air
    org.o2 = { 8 }
    sent = {}
    cond()
    local cw = last("clutchWound")
    check("short of air (o2 8): gasping (breathe, deep), both hands at the throat", bc.zcAir and last("breathe") and last("breathe").start == true
          and last("breathe").breathingScale > 1 and cw and cw.start and cw.numInjuries == 2, cw and cw.numInjuries)
    check("... not resting meanwhile", hook.Run("DeliriumMayRest", bc) == false)
    org.o2 = { 12 }
    cond()
    local airWas = bc.zcAir
    sent = {}
    cond()
    check("... still short at 12 (until it's over 15), and not started afresh each tick", bc.zcAir ~= nil and bc.zcAir == airWas and last("breathe") == nil)
    sent = {}
    ADVANCE(0.1)
    bc.lastHit = CurTime()
    ADVANCE(0.7)
    cond()
    check("... other damage (no reaction run: a prop on it, a scrape): the gasping carries on, not started afresh", last("breathe") == nil)
    bc.lastReact = CurTime()
    cond()
    local during = last("breathe")
    ADVANCE(0.7)
    bc.state.behaviours = { "dlShotReact" }
    cond()
    check("... the hit's own reaction still playing: not yet (its end would undo it)", last("breathe") == nil)
    bc.state.behaviours = {}
    ADVANCE(0.7)
    cond()
    check("... hit (its reaction stops everything): gasping again once it's played out", during == nil and last("breathe") and last("breathe").start == true,
          string.format("during %s, after %s, air %s", tostring(during and during.start), tostring(last("breathe") and last("breathe").start), tostring(bc.zcAir)))
    org.o2 = { 20 }
    sent = {}
    cond()
    check("breathing again (20): the gasping over, the hands let go", bc.zcAir == nil and last("breathe") and last("breathe").start == false
          and last("clutchWound") and last("clutchWound").start == false)
    local realPC, realCW = util.PointContents, CONTENTS_WATER
    CONTENTS_WATER = 32
    util.PointContents = function() return 32 end
    org.o2 = { 30 }
    sent = {}
    cond()
    check("the head under water, its air not running out yet (30): no struggling", bc.zcAir == nil and last("armsWindmillAdaptive") == nil)
    org.o2 = { 20 }
    sent = {}
    cond()
    check("under water: struggling up for air (the arms clawing, the legs kicking)", bc.zcAir and bc.zcAir.wet and last("armsWindmillAdaptive")
          and last("armsWindmillAdaptive").start and last("pedalLegs") and last("pedalLegs").start)
    util.PointContents = function() return 0 end
    sent = {}
    cond()
    check("... out of it: over", bc.zcAir == nil and last("armsWindmillAdaptive") and last("armsWindmillAdaptive").start == false and last("pedalLegs").start == false)
    util.PointContents, CONTENTS_WATER = realPC, realCW
    org.o2 = { 5 }
    D.ZCity.chokedBy = { [brag] = { IsValid = function() return true end, LookupBone = function() return nil end } }
    bc.zcChoke = { by = D.ZCity.chokedBy[brag], holds = {} }
    sent = {}
    cond()
    check("choked: its own (no gasping over it)", bc.zcAir == nil and last("breathe") == nil)
    D.ZCity.chokedBy, bc.zcChoke = nil, nil
    -- on an opioid (fentanyl): its low oxygen is its slowed breathing - no gasping
    org.analgesia = 1.6
    sent = {}
    cond()
    check("drugged, its oxygen low: no gasping, no hands at the throat (no urge to breathe)", bc.zcAir == nil and last("breathe") == nil)
    org.analgesia, org.o2 = 0, { 30 }
    -- a gun in its hand (and the other on its grip): those arms busy - kept strong, not braced with
    bc.zcGun = { gripping = true }
    cond()
    check("holding a gun two-handed: both arms busy", bc.armsBusy and bc.armsBusy[1] == true and bc.armsBusy[2] == true)
    bc.zcGun = { gripping = false }
    cond()
    check("... one-handed: the gun hand only", bc.armsBusy[1] == false and bc.armsBusy[2] == true)
    bc.zcGun = nil
    cond()
    check("... no gun: none", bc.armsBusy[1] == false and bc.armsBusy[2] == false)
    delirium.Message = realM
    delirium.Message(id, "stopAllBehaviours", {})

    -- a downed NPC: its head turned to whoever shot it
    sent = {}
    delirium.Message = function(i, name, params) sent[#sent + 1] = { name = name, params = params or {} } return realM(i, name, params) end
    local foe = { IsValid = function() return true end, EyePos = function() return Vector(200, 0, 64) end, GetClass = function() return "player" end,
                  IsPlayer = function() return true end, IsNPC = function() return false end, Alive = function() return true end,
                  GetPos = function() return Vector(200, 0, 0) end }
    local lc = { id = id, ent = brag, profile = { byName = {} }, lastHit = CurTime(), lastAttacker = foe }
    local threats = D.ZCity.ThreatsOf(lc, nil, Vector(0, 0, 40))
    check("whoever shot it lately: watched for", #threats == 1 and threats[1].ent == foe)
    D.ZCity.LookAround(lc, {}, threats, Vector(0, 0, 66))
    local hl = last("headLook")
    check("... it looks at them (in sight)", hl and hl.start and math.abs(hl.pos[1] - 200 * 0.0254) < 1e-3 and math.abs(hl.pos[3] - 64 * 0.0254) < 1e-3)
    local nlook = #sent
    D.ZCity.LookAround(lc, {}, threats, Vector(0, 0, 66))
    check("... not sent again while they're where they were", #sent == nlook)
    ADVANCE(0.1)
    lc.lastHit, lc.lastReact = CurTime(), CurTime()
    ADVANCE(0.7)
    D.ZCity.LookAround(lc, {}, threats, Vector(0, 0, 66))
    check("... hit (its reaction stops the look): looking again once it's played out", #sent > nlook and last("headLook").start == true)
    lc.unconscious = CurTime()
    D.ZCity.LookAround(lc, {}, threats, Vector(0, 0, 66))
    check("out cold: no longer looking", last("headLook").start == false and lc.zcLook == nil)
    lc.unconscious = nil
    ADVANCE(30)
    check("long since shot, nobody near: nobody watched for", #D.ZCity.ThreatsOf(lc, nil, Vector(0, 0, 40)) == 0)
    -- shot by an NPC that's down itself (hidden behind its body): its eyes are its body's head
    local downed = { IsValid = function() return true end, IsPlayer = function() return false end, IsNPC = function() return true end,
                     DeliriumReplaced = true, DeliriumBody = { ent = brag }, EyePos = function() return Vector(999, 999, 999) end, GetClass = function() return "npc_x" end }
    lc.lastAttacker, lc.lastHit = downed, CurTime()
    local dt = D.ZCity.ThreatsOf(lc, nil, Vector(0, 0, 40))
    check("shot by an NPC down itself: watched where its body's head is", #dt == 1 and dt[1].pos == Vector(0, 0, 66), dt[1] and tostring(dt[1].pos))
    -- shot by a player Z-City has ragdolled: their ragdoll's head, not the hidden player's eyes
    local downedPly = { IsValid = function() return true end, IsPlayer = function() return true end, IsNPC = function() return false end,
                        FakeRagdoll = brag, EyePos = function() return Vector(999, 999, 999) end, Alive = function() return true end, GetClass = function() return "player" end }
    lc.lastAttacker, lc.lastHit = downedPly, CurTime()
    dt = D.ZCity.ThreatsOf(lc, nil, Vector(0, 0, 40))
    check("shot by a player who's down (Z-City's ragdoll): watched where their ragdoll's head is", #dt == 1 and dt[1].pos == Vector(0, 0, 66) and dt[1].body == brag)
    -- aiming: it looks at what it aims at, not at whoever shot it
    local aimed = { IsValid = function() return true end, IsPlayer = function() return false end, IsNPC = function() return true end,
                    EyePos = function() return Vector(-300, 50, 60) end, GetClass = function() return "npc_y" end }
    local aimedHealth = 100
    aimed.Health = function() return aimedHealth end
    lc.lastAttacker, lc.zcGun = foe, { busy = true, target = aimed }
    sent = {}
    D.ZCity.LookAround(lc, {}, D.ZCity.ThreatsOf(lc, nil, Vector(0, 0, 40)), Vector(0, 0, 66))
    check("aiming: it looks at what it aims at, not at whoever shot it", last("headLook") and math.abs(last("headLook").pos[1] + 300 * 0.0254) < 1e-3)
    aimedHealth = 0
    sent = {}
    D.ZCity.LookAround(lc, {}, D.ZCity.ThreatsOf(lc, nil, Vector(0, 0, 40)), Vector(0, 0, 66))
    check("... killed it: no longer staring at the body - back to whoever shot it", last("headLook") and math.abs(last("headLook").pos[1] - 200 * 0.0254) < 1e-3)
    lc.zcGun = nil
    -- the ragdolls about (corpses, the downed): collected with the threats - seen past, not hidden behind
    local corpse = { IsValid = function() return true end, IsRagdoll = function() return true end, IsPlayer = function() return false end, IsNPC = function() return false end }
    local realFind = ents.FindInSphere
    ents.FindInSphere = function() return { corpse } end
    D.ZCity.ThreatsOf(lc, nil, Vector(0, 0, 40))
    check("the ragdolls about: kept to see past (a corpse isn't cover)", lc.zcSeeThrough and lc.zcSeeThrough[1] == corpse)
    ents.FindInSphere = realFind

    -- cover: in a world of boxes - the floor, and a crate (60 high) between it and the one shooting at it
    local BOXES = { { Vector(-4000, -4000, -50), Vector(4000, 4000, 0) }, { Vector(120, -100, 0), Vector(160, 100, 60) } }
    local function boxHit(a, b, lo, hi)
        if a.x > lo.x and a.x < hi.x and a.y > lo.y and a.y < hi.y and a.z > lo.z and a.z < hi.z then return 0, Vector(0, 0, 1), true end
        local t0, t1, nrm = 0, 1, nil
        for _, ax in ipairs({ "x", "y", "z" }) do
            local d = b[ax] - a[ax]
            if math.abs(d) < 1e-9 then
                if a[ax] < lo[ax] or a[ax] > hi[ax] then return nil end
            else
                local ta, tb = (lo[ax] - a[ax]) / d, (hi[ax] - a[ax]) / d
                local na, nb = Vector(0, 0, 0), Vector(0, 0, 0)
                na[ax], nb[ax] = -1, 1
                if ta > tb then ta, tb, na, nb = tb, ta, nb, na end
                if ta > t0 then t0, nrm = ta, na end
                if tb < t1 then t1 = tb end
                if t0 > t1 then return nil end
            end
        end
        if not nrm then return nil end
        return t0, nrm, false
    end
    local realTrace = util.TraceLine
    util.TraceLine = function(t)
        local best
        for _, bx in ipairs(BOXES) do
            local f, nrm, solid = boxHit(t.start, t.endpos, bx[1], bx[2])
            if f and (not best or f < best.f) then best = { f = f, n = nrm, solid = solid } end
        end
        if not best then return { Hit = false, HitPos = Vector(t.endpos.x, t.endpos.y, t.endpos.z), Fraction = 1, Entity = NULL } end
        return { Hit = true, StartSolid = best.solid, HitPos = t.start + (t.endpos - t.start) * best.f, HitNormal = best.n, Fraction = best.f, Entity = WORLD }
    end
    local shooter = { ent = foe, pos = Vector(400, 0, 64) }
    local cc = { id = id, ent = brag, profile = { byName = {} } }
    local here = Vector(0, 250, 8)
    check("(lying in the open: it can be seen)", not D.ZCity.HiddenFrom(cc, Vector(0, 250, 0), { shooter }))
    local spot = D.ZCity.CoverFor(cc, here, { shooter })
    check("down in the open: it drags itself into cover - a spot behind the crate, near, on the floor, out of their sight",
          spot and spot.x < 120 and spot:Distance(here) < 270 and math.abs(spot.z) < 1 and D.ZCity.HiddenFrom(cc, spot, { shooter }),
          spot and string.format("%.0f %.0f %.0f, %.0f units off", spot.x, spot.y, spot.z, spot:Distance(here)) or tostring(spot))
    check("... kept to on the way (no new search each tick)", spot and D.ZCity.CoverFor(cc, here, { shooter }) == spot)
    local there = spot and Vector(spot.x, spot.y, 8)
    check("... there: in cover, it stays", there and D.ZCity.CoverFor(cc, there, { shooter }) == false and cc.zcCover.reached)
    -- having got there, a little short of it (its pelvis in view), and a fresh look due: stays, not off to another spot.
    -- (at the shadow's edge: the last spot still hidden going out along y, and a point just past it in view)
    ADVANCE(2)
    local edge, past
    for y = 120, 260, 2 do
        local p = Vector(-87, y, 0)
        if D.ZCity.HiddenFrom(cc, p, { shooter }) then edge = p
        elseif edge and not past and p:Distance(edge) > 20 then past = p end
    end
    local ok = edge and past and past:Distance(edge) < 48
    if ok then cc.zcCover = { at = edge, t = CurTime(), reached = true } end
    check("... a little short of it, in view, a fresh look due: it stays in its cover (doesn't crawl off to the next)",
          ok and D.ZCity.CoverFor(cc, Vector(past.x, past.y, 8), { shooter }) == false,
          string.format("edge %s, past %s", tostring(edge), tostring(past)))
    -- they come round it (now shooting from the other side): its cover seen into - another looked for at once
    local flanker = { ent = foe, pos = Vector(-300, 400, 64) }
    if ok then
        cc.zcCover, cc.zcCoverNext = { at = edge, t = CurTime(), reached = true }, CurTime() + 10
        check("(its cover seen into from there)", not D.ZCity.HiddenFrom(cc, edge, { flanker }))
        local r = D.ZCity.CoverFor(cc, Vector(edge.x, edge.y, 8), { flanker })
        check("... seen into: looked for again at once, not kept to (nor waited on)", r ~= edge and not (cc.zcCover and cc.zcCover.at == edge),
              tostring(r))
    end
    BOXES[2] = nil
    ADVANCE(2)
    check("nothing to hide behind: none (it crawls away from them instead)", D.ZCity.CoverFor({ id = id, ent = brag, profile = { byName = {} } }, here, { shooter }) == nil)
    util.TraceLine = realTrace
    delirium.Message = realM
end)()
-- a downed NPC's body, a whole Z-City tick: who it's wary of (whoever shot it, an enemy near), its head turned to them,
-- dragging itself off; bleeding out past sinking, it stays down - not the NPC again
;(function()
    local sent, logged = {}, nil
    local realM, realLog = delirium.Message, D.log
    delirium.Message = function(i, name, params) sent[#sent + 1] = { name = name, params = params or {} } return realM(i, name, params) end
    D.log = function(fmt, ...) logged = string.format(fmt, ...) end
    local function last(name) for k = #sent, 1, -1 do if sent[k].name == name then return sent[k].params end end end
    local phys = { IsValid = function() return true end, GetPos = function() return Vector(0, 0, 8) end,
                   GetVelocity = function() return Vector(0, 0, 0) end, GetAngleVelocity = function() return Vector(0, 0, 0) end }
    local nrag = { IsValid = function() return true end, IsOnFire = function() return false end,
                   LookupBone = function(_, name) return name == "ValveBiped.Bip01_Head1" and 1 or nil end,
                   GetBonePosition = function() return Vector(0, 30, 10), Angle(0, 0, 0) end, GetPhysicsObjectNum = function() return phys end }
    local foe = { IsValid = function() return true end, EyePos = function() return Vector(300, 0, 64) end, GetClass = function() return "player" end,
                  IsPlayer = function() return true end, IsNPC = function() return false end, Alive = function() return true end,
                  GetPos = function() return Vector(300, 0, 0) end, WorldSpaceCenter = function() return Vector(300, 0, 36) end }
    local norg = { fake = true, blood = 5000, pain = 0, o2 = { 30 } }
    local npc = { IsValid = function() return true end, organism = norg, GetClass = function() return "npc_test" end, Disposition = function() return 1 end }
    local nc = { id = id, ent = nrag, profile = { byName = {} }, root = 0, lastHit = CurTime(), lastAttacker = foe, externalHealth = true, zcNpc = npc,
                 npc = { class = "npc_test", health = 40 } }
    nc.state = { balancing = false, extras = {}, behaviours = {} }
    delirium.Message(id, "stopAllBehaviours", {})
    local realState = delirium.State
    delirium.State = function(i) if i == id then return nc.state end return realState(i) end
    D.chars[nrag] = nc
    hook.t.Tick.DeliriumZCity()
    local hl, cr = last("headLook"), last("learnedCrawl")
    check("a downed NPC's tick: no error", logged == nil, logged)
    check("... its head turned to whoever shot it", hl and hl.start and math.abs(hl.pos[1] - 300 * 0.0254) < 1e-3, hl and hl.pos[1])
    check("... dragging itself off (nowhere to hide here: away from them)", cr and cr.start and cr.targetPosition[1] < 0,
          cr and string.format("towards %.2f %.2f", cr.targetPosition[1], cr.targetPosition[2]) or "no crawl")
    norg.blood = 3000
    ADVANCE(0.2)
    hook.t.Tick.DeliriumZCity()
    check("bleeding out past sinking: it stays down, not the NPC again", nc.noGetUp == true and hook.Run("DeliriumMayGetUp", nc) == false and logged == nil, logged)
    norg.pain = 70
    ADVANCE(0.2)
    hook.t.Tick.DeliriumZCity()
    check("bleeding, curled up in pain: at full strength (weaker, the curl wouldn't show)", nc.zcCurl ~= nil and nc.strengthScale == nil and logged == nil,
          tostring(nc.strengthScale))
    -- lying hurt, nobody after it (no crawl): writhing; hit, its reaction ends that - writhing again once it's played out
    norg.pain, norg.blood = 0, 5000
    nc.lastAttacker, nc.zcCurl, nc.zcCrawl = nil, nil, nil
    delirium.Message(id, "bodyFoetal", { start = false })
    ADVANCE(0.2)
    hook.t.Tick.DeliriumZCity()
    ADVANCE(0.2)
    hook.t.Tick.DeliriumZCity()
    check("lying hurt, nobody after it: writhing (injuredOnGround)", nc.zcGround == true and last("injuredOnGround") and last("injuredOnGround").start == true and logged == nil, logged)
    ADVANCE(0.2)
    nc.lastReact = CurTime()
    sent = {}
    hook.t.Tick.DeliriumZCity()
    local during = last("injuredOnGround")
    ADVANCE(0.7)
    hook.t.Tick.DeliriumZCity()
    check("... hit (its reaction ends the writhing): writhing again once it's played out", during == nil and last("injuredOnGround") and last("injuredOnGround").start == true)
    -- carried (Z-City's hands): picked up, and not writhing over the carry
    ADVANCE(0.2)
    nc.held, nc.lastReact = 0, CurTime()
    sent = {}
    ADVANCE(0.7)
    hook.t.Tick.DeliriumZCity()
    check("carried (picked up: it stops all): not writhing over the carry", last("injuredOnGround") == nil)
    nc.held = nil
    D.chars[nrag] = nil
    delirium.State, delirium.Message, D.log = realState, realM, realLog
    delirium.Message(id, "stopAllBehaviours", {})
end)()
-- Z-City's own pulls on a ragdolled player's bones: an arm it aims or grabs with, the head it turns to where they look -
-- let go of while it does (the muscles would only fight it); the trunk, not
;(function()
    local names = { [0] = "ValveBiped.Bip01_Pelvis", "ValveBiped.Bip01_Head1", "ValveBiped.Bip01_R_Hand" }
    local phys, prag = {}, {}
    for i = 0, 2 do
        phys[i] = setmetatable({ GetEntity = function() return prag end }, { __index = PHYSMETA })
    end
    prag.IsValid = function() return true end
    prag.GetPhysicsObjectNum = function(_, i) return phys[i] end
    local bones = {}
    for i = 0, 2 do bones[i] = { name = names[i] } end
    local pc = { id = id, ent = prag, profile = { count = 3, bones = bones, byName = {} } }
    D.chars[prag] = pc
    local watching = D.ZCity.watching
    D.ZCity.watching = true
    local pull = { pos = Vector(0, 0, 50), angle = Angle(0, 0, 0), secondstoarrive = 0.1, teleportdistance = 1, maxangular = 100, maxspeed = 60 }
    for i = 0, 2 do PHYSMETA.ComputeShadowControl(phys[i], pull) end
    local loose = pc.zcLoose or {}
    check("Z-City turning a ragdolled player's head (and pulling an arm): those let go of while it does; the pelvis not",
          loose[1] ~= nil and loose[2] ~= nil and loose[0] == nil)
    D.chars[prag] = nil
    D.ZCity.watching = watching
end)()
-- a hand on a wound: the last few centimetres the game's physics' - drawn onto the wound itself, its palm on the skin
;(function()
    local U = 1 / 0.0254
    local calls = {}
    local function hand(at)
        local h = { pos = at, calls = {} }
        h.IsValid = function() return true end
        h.GetPos = function(self) return Vector(self.pos.x, self.pos.y, self.pos.z) end
        h.GetAngles = function() return Angle(0, 0, 0) end
        h.GetMassCenter = function() return Vector(1, 0, 0) end
        h.LocalToWorld = function(self, v) return self.pos + v end
        h.Wake = function() end
        h.ComputeShadowControl = function(self, p) self.calls[#self.calls + 1] = p end
        return h
    end
    local lh, rh = hand(Vector(5, 2, 50)), hand(Vector(30, -10, 50))
    local bones = { ["ValveBiped.Bip01_Spine2"] = { 1, Vector(0, 0, 50) }, ["ValveBiped.Bip01_L_Forearm"] = { 2, Vector(0, 12, 50) } }
    local byIndex = { [1] = "ValveBiped.Bip01_Spine2", [2] = "ValveBiped.Bip01_L_Forearm" }
    local srag = { IsValid = function() return true end,
                   LookupBone = function(_, n) return bones[n] and bones[n][1] or nil end,
                   GetBonePosition = function(_, b) local n = byIndex[b] return n and bones[n][2] or nil, Angle(0, 0, 0) end,
                   GetPhysicsObjectNum = function(_, i) return (i == 16 and lh) or (i == 20 and rh) or nil end }
    local wound = { 1.5, Vector(4, 0, 0), Angle(0, 0, 0), "ValveBiped.Bip01_Spine2", CurTime() }
    local sc = { id = id, ent = srag, profile = { byName = { ["ValveBiped.Bip01_L_Hand"] = { index = 16 }, ["ValveBiped.Bip01_R_Hand"] = { index = 20 } } },
                 zcClutchKey = "k", zcHeldWounds = { wound } }
    local ok, err = pcall(D.ZCity.SettleHands, sc)
    local p = lh.calls[1]
    -- (the wound at 4,0,50 facing +x: the palm 1 cm out on it; the palm is the hand's origin + 1,0,0)
    local want = Vector(4 + 0.01 * U - 1, 0, 50)
    check("a hand on a chest wound (5 cm off): drawn onto the wound itself, gently, the palm on the skin", ok and p and p.delirium == true and p.maxangular == 0
          and p.pos:Distance(want) < 0.01 and #rh.calls == 0, ok and (p and tostring(p.pos) or "no pull") or err)
    -- a hand far off: GTA IV's reach brings it first
    lh.pos, lh.calls = Vector(40, 0, 50), {}
    pcall(D.ZCity.SettleHands, sc)
    check("... far off (40 units): left to the reach - not dragged across", #lh.calls == 0 and #rh.calls == 0)
    -- a wound on the left forearm: held with the right hand, never its own
    wound[4], wound[2] = "ValveBiped.Bip01_L_Forearm", Vector(0, -2, 0)
    lh.pos, rh.pos = Vector(1, 10, 50), Vector(4, 8, 50)
    lh.calls, rh.calls = {}, {}
    pcall(D.ZCity.SettleHands, sc)
    check("a wound on the left forearm: the right hand on it, not the left's own", #lh.calls == 0 and #rh.calls == 1)
    -- out cold, or the cvar off: nothing
    rh.calls = {}
    sc.unconscious = CurTime()
    pcall(D.ZCity.SettleHands, sc)
    sc.unconscious = nil
    SET_CVAR("delirium_zcity_settle_holds", "0")
    pcall(D.ZCity.SettleHands, sc)
    SET_CVAR("delirium_zcity_settle_holds", "1")
    check("out cold, or delirium_zcity_settle_holds 0: left alone", #rh.calls == 0)
end)()
-- ragdoll combat: hit while Z-City walks the ragdoll - the walk let go of, and the body the hit's reaction's (not
-- balanced afresh at once, which would wipe the reaction out); after the stagger, balanced again
;(function()
    local called = {}
    local realBal, realCatch = delirium.Balance, delirium.CatchFall
    delirium.Balance = function(i, set) called[#called + 1] = "Balance" return realBal(i, set) end
    delirium.CatchFall = function(i, set) called[#called + 1] = "CatchFall" return realCatch(i, set) end
    local wrag = { IsValid = function() return true end, IsOnFire = function() return false end, LookupBone = function() return nil end,
                   GetBonePosition = function() return nil end, GetPhysicsObjectNum = function() return nil end }
    local ply = { IsValid = function() return true end, Alive = function() return true end, organism = { pain = 0, blood = 5000, o2 = { 30 } },
                  KeyDown = function() return false end, EyeAngles = function() return Angle(0, 0, 0) end, Nick = function() return "p" end }
    ply.FakeRagdoll = wrag
    local wc = { id = id, ent = wrag, profile = { byName = {} }, root = 0, lastHit = CurTime(), zcPlayer = ply, zcWalking = true }
    wc.state = { balancing = false, extras = {}, behaviours = {} }
    wc.zcStaggerUntil = CurTime() + 1
    D.chars[wrag] = wc
    local ok, err = pcall(hook.t.Tick.DeliriumZCity)
    check("ragdoll combat, hit: the walk let go of, the body left to the hit's reaction (no fresh balance over it)", ok and wc.zcWalking == nil and #called == 0,
          ok and table.concat(called, " ") or err)
    wc.zcWalking = true
    ADVANCE(1.2)
    wc.zcTick = nil
    pcall(hook.t.Tick.DeliriumZCity)
    check("... the walk stopping otherwise: balanced (or caught) afresh as before", #called == 1, table.concat(called, " "))
    D.chars[wrag] = nil
    delirium.Balance, delirium.CatchFall = realBal, realCatch
    delirium.Message(id, "stopAllBehaviours", {})
end)()
OUT(failed == 0 and "all passed" or (failed .. " FAILED"))
