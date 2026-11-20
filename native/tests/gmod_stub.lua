-- A stand-in for the bits of Garry's Mod the Delirium addon calls, for running its real Lua against the real module
-- outside the game (python host.py gmod_stub.lua core.lua). The world is a floor at z = 0; ragdolls are ValveBiped
-- skeletons whose 15 physics bones carry the rest, and follow ComputeShadowControl's target exactly.

RESULT = ""
local function out(s) RESULT = RESULT .. s .. "\n" end
OUT = out
print = function(...) local t = {} for i = 1, select("#", ...) do t[i] = tostring((select(i, ...))) end out(table.concat(t, "\t")) end
SERVER, CLIENT = true, false

------------------------------------------------------------------------------------------------------------------------
-- Vectors and angles (Source's conventions)
------------------------------------------------------------------------------------------------------------------------
local V = {}
V.__index = V
function Vector(x, y, z)
    if type(x) == "table" then return setmetatable({ x = x.x, y = x.y, z = x.z }, V) end
    return setmetatable({ x = x or 0, y = y or 0, z = z or 0 }, V)
end
V.__add = function(a, b) return Vector(a.x + b.x, a.y + b.y, a.z + b.z) end
V.__sub = function(a, b) return Vector(a.x - b.x, a.y - b.y, a.z - b.z) end
V.__mul = function(a, b)
    if type(a) == "number" then return Vector(b.x * a, b.y * a, b.z * a) end
    if type(b) == "number" then return Vector(a.x * b, a.y * b, a.z * b) end
    return Vector(a.x * b.x, a.y * b.y, a.z * b.z)
end
V.__div = function(a, b) return Vector(a.x / b, a.y / b, a.z / b) end
V.__unm = function(a) return Vector(-a.x, -a.y, -a.z) end
V.__eq = function(a, b) return a.x == b.x and a.y == b.y and a.z == b.z end
V.__tostring = function(a) return string.format("%.2f %.2f %.2f", a.x, a.y, a.z) end
function V:Length() return math.sqrt(self.x * self.x + self.y * self.y + self.z * self.z) end
function V:LengthSqr() return self.x * self.x + self.y * self.y + self.z * self.z end
function V:Dot(b) return self.x * b.x + self.y * b.y + self.z * b.z end
function V:Cross(b) return Vector(self.y * b.z - self.z * b.y, self.z * b.x - self.x * b.z, self.x * b.y - self.y * b.x) end
function V:GetNormalized() local l = self:Length() if l < 1e-9 then return Vector(0, 0, 0) end return self / l end
function V:Normalize() local l = self:Length() if l > 1e-9 then self.x, self.y, self.z = self.x / l, self.y / l, self.z / l end end
function V:Distance(b) return (self - b):Length() end
function V:DistToSqr(b) return (self - b):LengthSqr() end
-- VectorAngles(forward, pseudoup)
function V:AngleEx(up)
    local f = self:GetNormalized()
    local left = up:Cross(f):GetNormalized()
    local xy = math.sqrt(f.x * f.x + f.y * f.y)
    local p, y, r
    if xy > 0.001 then
        y = math.deg(math.atan2(f.y, f.x))
        p = math.deg(math.atan2(-f.z, xy))
        local upz = left.y * f.x - left.x * f.y
        r = math.deg(math.atan2(left.z, upz))
    else
        y = math.deg(math.atan2(-left.x, left.y))
        p = math.deg(math.atan2(-f.z, xy))
        r = 0
    end
    return Angle(p, y, r)
end
function V:Angle() return self:AngleEx(Vector(0, 0, 1)) end
vector_origin = Vector(0, 0, 0)

local A = {}
A.__index = A
function Angle(p, y, r) return setmetatable({ p = p or 0, y = y or 0, r = r or 0 }, A) end
A.__add = function(a, b) return Angle(a.p + b.p, a.y + b.y, a.r + b.r) end
A.__sub = function(a, b) return Angle(a.p - b.p, a.y - b.y, a.r - b.r) end
function A:Forward()
    local p, y = math.rad(self.p), math.rad(self.y)
    return Vector(math.cos(p) * math.cos(y), math.cos(p) * math.sin(y), -math.sin(p))
end
function A:Right()
    local p, y, r = math.rad(self.p), math.rad(self.y), math.rad(self.r)
    local sp, cp, sy, cy, sr, cr = math.sin(p), math.cos(p), math.sin(y), math.cos(y), math.sin(r), math.cos(r)
    return Vector(-sr * sp * cy + cr * sy, -sr * sp * sy - cr * cy, -sr * cp)
end
function A:Up()
    local p, y, r = math.rad(self.p), math.rad(self.y), math.rad(self.r)
    local sp, cp, sy, cy, sr, cr = math.sin(p), math.cos(p), math.sin(y), math.cos(y), math.sin(r), math.cos(r)
    return Vector(cr * sp * cy + sr * sy, cr * sp * sy - sr * cy, cr * cp)
end
-- (turned in place by deg about a world axis, as Source's: its forward and up turned, the angle made again from them)
function A:RotateAroundAxis(axis, deg)
    local k = axis:GetNormalized()
    local th = math.rad(deg)
    local c, s = math.cos(th), math.sin(th)
    local function turn(v) return v * c + k:Cross(v) * s + k * (k:Dot(v) * (1 - c)) end
    local a = turn(self:Forward()):AngleEx(turn(self:Up()))
    self.p, self.y, self.r = a.p, a.y, a.r
end
A.__index = function(t, k) if k == "y" or k == "p" or k == "r" then return rawget(t, k) end return A[k] end
angle_zero = Angle(0, 0, 0)

-- quaternions <-> angles, for the stand-in's own sums
local function qOf(ang)
    local f, r, u = ang:Forward(), ang:Right(), ang:Up()
    local DM = STUB_DM
    return DM.fromAxes(DM.v(f.x, f.y, f.z), DM.v(-r.x, -r.y, -r.z), DM.v(u.x, u.y, u.z))
end
local function angOf(q)
    local DM = STUB_DM
    local X, _, Z = DM.axes(q)
    return Vector(X.x, X.y, X.z):AngleEx(Vector(Z.x, Z.y, Z.z))
end

function WorldToLocal(pos, ang, opos, oang)
    local DM = STUB_DM
    local qo, q = qOf(oang), qOf(ang)
    local d = DM.qrot(DM.qinv(qo), DM.v(pos.x - opos.x, pos.y - opos.y, pos.z - opos.z))
    return Vector(d.x, d.y, d.z), angOf(DM.qmul(DM.qinv(qo), q))
end
function LocalToWorld(pos, ang, opos, oang)
    local DM = STUB_DM
    local qo = qOf(oang)
    local d = DM.qrot(qo, DM.v(pos.x, pos.y, pos.z))
    return Vector(opos.x + d.x, opos.y + d.y, opos.z + d.z), angOf(DM.qmul(qo, qOf(ang)))
end

------------------------------------------------------------------------------------------------------------------------
-- The library
------------------------------------------------------------------------------------------------------------------------
local now = 100
function CurTime() return now end
function SysTime() return now end
function ADVANCE(dt) now = now + dt end
engine = { TickInterval = function() return 1 / 66 end, TickCount = function() return math.floor(now * 66) end }
function Color(r, g, b, a) return { r = r, g = g, b = b, a = a } end
color_white = Color(255, 255, 255)
function MsgC(...) local t = {} for _, x in ipairs({ ... }) do if type(x) == "string" then t[#t + 1] = x end end out(table.concat(t)) end
function ErrorNoHalt(s) out("ERROR: " .. s) end
function IsValid(x) return x ~= nil and x ~= false and type(x) == "table" and x.IsValid ~= nil and x:IsValid() == true end
function isentity(x) return type(x) == "table" and x._ent == true end
function istable(x) return type(x) == "table" end
function isvector(x) return type(x) == "table" and getmetatable(x) == V end
function isstring(x) return type(x) == "string" end
function isfunction(x) return type(x) == "function" end
function isnumber(x) return type(x) == "number" end
function isangle(x) return type(x) == "table" and x.p ~= nil and x.y ~= nil and x.r ~= nil end
math.Rand = math.Rand or function(a, b) return a + math.random() * (b - a) end
math.Clamp = function(x, lo, hi) return math.min(math.max(x, lo), hi) end
math.Approach = function(c, t, i) i = math.abs(i) if c < t then return math.min(c + i, t) end return math.max(c - i, t) end
table.Count = function(t) local n = 0 for _ in pairs(t) do n = n + 1 end return n end
table.HasValue = function(t, v) for _, x in pairs(t) do if x == v then return true end end return false end
string.StartWith = function(s, p) return string.sub(s, 1, #p) == p end
NULL = setmetatable({ _ent = true }, { __index = function() return function() return false end end })

local convars = {}
function CreateConVar(name, default)
    local c = { v = tostring(default) }
    function c:GetBool() return (tonumber(self.v) or 0) ~= 0 end
    function c:GetFloat() return tonumber(self.v) or 0 end
    function c:GetInt() return math.floor(tonumber(self.v) or 0) end
    function c:GetString() return self.v end
    convars[name] = c
    return c
end
function GetConVar(name) return convars[name] end
function SET_CVAR(name, val) convars[name].v = tostring(val) end

hook = { t = {} }
function hook.Add(ev, id, fn) hook.t[ev] = hook.t[ev] or {} hook.t[ev][id] = fn end
function hook.Remove(ev, id) if hook.t[ev] then hook.t[ev][id] = nil end end
function hook.Run(ev, ...)
    for _, fn in pairs(hook.t[ev] or {}) do
        local a, b, c = fn(...)
        if a ~= nil then return a, b, c end
    end
end
TIMERS = {}
timer = { Simple = function(t, fn) TIMERS[#TIMERS + 1] = { at = now + t, fn = fn } end, Create = function() end, Remove = function() end,
          Exists = function() return false end }
function RUN_TIMERS()
    local due, keep = {}, {}
    for _, t in ipairs(TIMERS) do if t.at <= now then due[#due + 1] = t else keep[#keep + 1] = t end end
    TIMERS = keep
    for _, t in ipairs(due) do t.fn() end
end
concommand = { Add = function() end }
-- (the host compiled every addon file into SOURCES; include looks in the addon being run, INCLUDE_ADDON)
INCLUDE_ADDON = "delirium"
function include(path)
    local f = SOURCES[INCLUDE_ADDON .. "/" .. path] or SOURCES["delirium/" .. path]
    if not f then error("include: no " .. path) end
    return f()
end
function AddCSLuaFile() end
function require(name) if name == "delirium" then return delirium end end

MASK_SOLID, MASK_NPCSOLID, MASK_SHOT = 1, 2, 3
COLLISION_GROUP_NONE, COLLISION_GROUP_WEAPON, COLLISION_GROUP_NPC = 0, 11, 9
DMG_GENERIC, DMG_CRUSH, DMG_BULLET, DMG_SLASH, DMG_BURN, DMG_VEHICLE, DMG_FALL, DMG_BLAST, DMG_CLUB = 0, 1, 2, 4, 8, 16, 32, 64, 128
DMG_SHOCK, DMG_SONIC, DMG_DROWN, DMG_PARALYZE, DMG_NERVEGAS, DMG_POISON, DMG_RADIATION = 256, 512, 16384, 32768, 65536, 131072, 262144
DMG_DROWNRECOVER, DMG_SLOWBURN, DMG_REMOVENORAGDOLL, DMG_PHYSGUN, DMG_DISSOLVE, DMG_BUCKSHOT = 524288, 2097152, 4194304, 8388608, 67108864, 536870912
HITGROUP_GENERIC, HITGROUP_HEAD, HITGROUP_CHEST, HITGROUP_STOMACH = 0, 1, 2, 3
HITGROUP_LEFTARM, HITGROUP_RIGHTARM, HITGROUP_LEFTLEG, HITGROUP_RIGHTLEG = 4, 5, 6, 7
MOVETYPE_NONE, MOVETYPE_WALK, MOVETYPE_NOCLIP, MOVETYPE_OBSERVER = 0, 2, 8, 10
FSOLID_NOT_SOLID, FL_NOTARGET, EFL_NO_THINK_FUNCTION = 4, 1, 2
SCHED_IDLE_STAND, ACT_IDLE = 1, 1

-- the world: a floor at z = 0
local function floorHit(a, b, lift)
    lift = lift or 0
    local az, bz = a.z - lift, b.z - lift
    if az < 0 then return { Hit = true, StartSolid = true, HitPos = Vector(a), HitNormal = Vector(0, 0, 1), Entity = WORLD, Fraction = 0 } end
    if bz > 0 then return { Hit = false, HitPos = Vector(b), Fraction = 1, Entity = NULL } end
    local f = az / (az - bz)
    local p = a + (b - a) * f
    return { Hit = true, HitPos = p, HitNormal = Vector(0, 0, 1), Entity = WORLD, Fraction = f }
end
TRACES = 0
util = {
    TraceLine = function(t) TRACES = TRACES + 1 return floorHit(t.start, t.endpos) end,
    TraceHull = function(t) TRACES = TRACES + 1 return floorHit(t.start, t.endpos, t.mins and t.mins.z or 0) end,
    AddNetworkString = function(name) NETSTRINGS = NETSTRINGS or {} NETSTRINGS[name] = true end,
}
-- net: messages sent are kept (NETSENT), receivers too (NETRECV); NET_DELIVER(msg) hands one to its receiver
NETSENT, NETRECV = {}, {}
local building
net = {
    Start = function(name) building = { name = name, values = {} } end,
    WriteEntity = function(v) table.insert(building.values, v) end,
    WriteVector = function(v) table.insert(building.values, v) end,
    WriteAngle = function(v) table.insert(building.values, v) end,
    WriteBool = function(v) table.insert(building.values, v) end,
    Broadcast = function() table.insert(NETSENT, building) end,
    Send = function() table.insert(NETSENT, building) end,
    Receive = function(name, fn) NETRECV[name] = fn end,
}
local reading
net.ReadEntity = function() return table.remove(reading, 1) end
net.ReadVector, net.ReadAngle, net.ReadBool = net.ReadEntity, net.ReadEntity, net.ReadEntity
function NET_DELIVER(msg) reading = { unpack(msg.values) } NETRECV[msg.name]() end

local function newEnt(t)
    t._ent = true
    t.valid = true
    t.nw = {}
    function t:IsValid() return self.valid end
    function t:IsWorld() return self.kind == "world" end
    function t:IsPlayer() return false end
    function t:IsNPC() return self.kind == "npc" end
    function t:IsVehicle() return false end
    function t:IsRagdoll() return self.class == "prop_ragdoll" end
    function t:IsWeapon() return false end
    function t:GetClass() return self.class end
    function t:SetNWBool(k, v) self.nw[k] = v end
    function t:Remove()
        if not self.valid then return end
        self.valid = false
        for _, fn in pairs(self.onRemove or {}) do fn(self) end
    end
    function t:CallOnRemove(k, fn) self.onRemove = self.onRemove or {} self.onRemove[k] = fn end
    function t:RemoveCallOnRemove(k) if self.onRemove then self.onRemove[k] = nil end end
    return t
end

WORLD = newEnt({ kind = "world", class = "worldspawn" })
function WORLD:GetPhysicsObject()
    local s = 4000
    -- a slab z in [-50, 0]: 12 triangles
    local c = {}
    local function q(a, b, cc, d) for _, p in ipairs({ a, b, cc, a, cc, d }) do c[#c + 1] = { pos = p } end end
    local lo, hi = -50, 0
    local P = function(x, y, z) return Vector(x, y, z) end
    q(P(-s, -s, hi), P(s, -s, hi), P(s, s, hi), P(-s, s, hi))
    q(P(-s, -s, lo), P(-s, s, lo), P(s, s, lo), P(s, -s, lo))
    q(P(-s, -s, lo), P(s, -s, lo), P(s, -s, hi), P(-s, -s, hi))
    q(P(s, -s, lo), P(s, s, lo), P(s, s, hi), P(s, -s, hi))
    q(P(s, s, lo), P(-s, s, lo), P(-s, s, hi), P(s, s, hi))
    q(P(-s, s, lo), P(-s, -s, lo), P(-s, -s, hi), P(-s, s, hi))
    return { IsValid = function() return true end, GetMeshConvexes = function() return { c } end }
end
game = { GetWorld = function() return WORLD end, SinglePlayer = function() return true end }
-- constraints: welds made are kept (WELDS), as entities (removing one breaks it)
WELDS = {}
constraint = {
    Weld = function(e1, e2, b1, b2, force, nocollide, deleteOnBreak)
        local w = newEnt({ kind = "constraint", class = "phys_constraint", ent1 = e1, ent2 = e2, bone1 = b1, bone2 = b2, force = force })
        WELDS[#WELDS + 1] = w
        return w
    end,
}
player = { GetAll = function() return {} end }

------------------------------------------------------------------------------------------------------------------------
-- ValveBiped ragdolls
------------------------------------------------------------------------------------------------------------------------
local B = "ValveBiped.Bip01_"
-- name, parent, the Delirium part whose joint it is (its rest position comes from Homer's, turned to face +X, scaled)
SKELETON = {
    { B .. "Pelvis", nil, 0 }, { B .. "Spine", B .. "Pelvis", 7 }, { B .. "Spine1", B .. "Spine", 8 }, { B .. "Spine2", B .. "Spine1", 9 },
    { B .. "Spine4", B .. "Spine2", 10 }, { B .. "Neck1", B .. "Spine4", 11 }, { B .. "Head1", B .. "Neck1", 12 },
    { B .. "L_Clavicle", B .. "Spine4", 13 }, { B .. "L_UpperArm", B .. "L_Clavicle", 14 }, { B .. "L_Forearm", B .. "L_UpperArm", 15 },
    { B .. "L_Hand", B .. "L_Forearm", 16 }, { B .. "R_Clavicle", B .. "Spine4", 17 }, { B .. "R_UpperArm", B .. "R_Clavicle", 18 },
    { B .. "R_Forearm", B .. "R_UpperArm", 19 }, { B .. "R_Hand", B .. "R_Forearm", 20 },
    { B .. "L_Thigh", B .. "Pelvis", 1 }, { B .. "L_Calf", B .. "L_Thigh", 2 }, { B .. "L_Foot", B .. "L_Calf", 3 },
    { B .. "R_Thigh", B .. "Pelvis", 4 }, { B .. "R_Calf", B .. "R_Thigh", 5 }, { B .. "R_Foot", B .. "R_Calf", 6 },
}
PHYS = { "Pelvis", "Spine2", "R_UpperArm", "L_UpperArm", "L_Forearm", "L_Hand", "R_Forearm", "R_Hand", "R_Thigh", "R_Calf", "Head1",
         "L_Thigh", "L_Calf", "L_Foot", "R_Foot" }

-- The stand-in model's rest skeleton: Homer's joints (from the module), turned to face +X, 'scale' times as big, the
-- arms raised into an A pose by 'armRaise' degrees (so the rest poses differ, as a real model's does). Each bone's axes:
-- X along it (to its first child, a leaf: its parent's), Z the body's forward made square to X (feet: up).
function MAKE_MODEL(name, scale, armRaise)
    local DM = STUB_DM
    local f = delirium.RestPose("")
    local info = delirium.PartInfo("")
    local frames = DM.unpackFrames(f)
    local anchors = {}
    for k = 0, 20 do anchors[k] = DM.v(info[k * 7 + 5], info[k * 7 + 6], info[k * 7 + 7]) end
    local piv = DM.pivots(frames, anchors)
    local turn = DM.qyaw(math.pi / 2 - (delirium.Info().restYaw))
    local pos = {}
    for _, b in ipairs(SKELETON) do
        local p = DM.qrot(turn, piv[b[3]])
        pos[b[1]] = Vector(p.x, p.y, p.z) * (DM.UNITS * scale)
    end
    -- raise each arm about the shoulder (UpperArm joint) around the body's forward (+X)
    for _, side in ipairs({ { "L", 1 }, { "R", -1 } }) do
        local s, sign = side[1], side[2]
        local sh = pos[B .. s .. "_UpperArm"]
        local a = math.rad(armRaise) * sign
        for _, n in ipairs({ "_Forearm", "_Hand" }) do
            local d = pos[B .. s .. n] - sh
            local y = d.y * math.cos(a) - d.z * math.sin(a)
            local z = d.y * math.sin(a) + d.z * math.cos(a)
            pos[B .. s .. n] = sh + Vector(d.x, y, z)
        end
    end
    local kids = {}
    for _, b in ipairs(SKELETON) do if b[2] then kids[b[2]] = kids[b[2]] or b[1] end end
    local rest = {}
    for _, b in ipairs(SKELETON) do
        local along
        if kids[b[1]] then along = pos[kids[b[1]]] - pos[b[1]] else along = pos[b[1]] - pos[b[2]] end
        local second = string.find(b[1], "Foot") and Vector(0, 0, 1) or Vector(1, 0, 0)
        if math.abs(along:GetNormalized():Dot(second)) > 0.95 then second = Vector(0, 1, 0) end
        rest[b[1]] = { pos = pos[b[1]], ang = along:AngleEx(second - along:GetNormalized() * along:GetNormalized():Dot(second)) }
    end
    MODELS = MODELS or {}
    MODELS[name] = rest
end

local function ragdoll(model)
    local rest = MODELS[model]
    local r = newEnt({ kind = "ragdoll", class = "prop_ragdoll", model = model, phys = {}, nwv = {} })
    r.base, r.baseAng = Vector(0, 0, 0), Angle(0, 0, 0)
    local function place()
        r.phys = {}
        for i, n in ipairs(PHYS) do
            local b = rest and rest[B .. n]
            if b then
                local p, a = LocalToWorld(b.pos, b.ang, r.base, r.baseAng)
                r.phys[i - 1] = MAKE_PHYS(p, a, B .. n)
            end
        end
    end
    function r:SetModel(m) self.model = m rest = MODELS[m] end
    function r:GetModel() return self.model end
    function r:SetPos(p) self.base = p end
    function r:SetAngles(a) self.baseAng = a end
    function r:Spawn() place() end
    function r:Activate() end
    function r:GetPos() return self.phys[0] and self.phys[0]:GetPos() or self.base end
    function r:WorldSpaceCenter() return self:GetPos() end
    function r:GetPhysicsObjectCount() local n = 0 for _ in pairs(self.phys) do n = n + 1 end return n end
    function r:GetPhysicsObjectNum(i) return self.phys[i] end
    function r:LookupBone(name) for i, b in ipairs(SKELETON) do if b[1] == name then return i - 1 end end return nil end
    function r:GetBoneName(id) return SKELETON[id + 1] and SKELETON[id + 1][1] end
    function r:GetBoneParent(id)
        local p = SKELETON[id + 1] and SKELETON[id + 1][2]
        return p and self:LookupBone(p) or -1
    end
    function r:TranslatePhysBoneToBone(i) return self:LookupBone(B .. PHYS[i + 1]) end
    function r:TranslateBoneToPhysBone(id)
        local name = self:GetBoneName(id)
        for i, n in ipairs(PHYS) do if B .. n == name then return i - 1 end end
        return -1
    end
    function r:SetupBones() end
    -- a bone rides on its nearest physics bone, as at rest
    function r:GetBoneMatrix(id)
        local name = self:GetBoneName(id)
        if not name or not rest then return nil end
        local cur = id
        while cur and cur >= 0 do
            local pi = self:TranslateBoneToPhysBone(cur)
            if pi >= 0 then
                local phys = self.phys[pi]
                local pb = rest[self:GetBoneName(cur)]
                local lp, la = WorldToLocal(rest[name].pos, rest[name].ang, pb.pos, pb.ang)
                local p, a = LocalToWorld(lp, la, phys:GetPos(), phys:GetAngles())
                return { GetTranslation = function() return p end, GetAngles = function() return a end }
            end
            cur = self:GetBoneParent(cur)
        end
    end
    function r:SetCustomCollisionCheck() end
    function r:CollisionRulesChanged() end
    function r:SetCollisionGroup(g) self.group = g end
    function r:AddCallback() return 1 end
    function r:RemoveCallback() end
    function r:SetSkin() end
    function r:GetSkin() return 0 end
    function r:GetBodyGroups() return {} end
    function r:SetMaterial() end
    function r:ManipulateBoneAngles(b, a) self.boneAng = self.boneAng or {} self.boneAng[b] = a end
    function r:EmitSound(snd) self.sounds = self.sounds or {} self.sounds[#self.sounds + 1] = snd end
    function r:GetMaterial() return "" end
    function r:SetColor() end
    function r:GetColor() return Color(255, 255, 255) end
    function r:SetNoDraw(b) self.nodraw = b end
    function r:GetNoDraw() return self.nodraw end
    return r
end

SHADOW_CALLS = 0
function MAKE_PHYS(pos, ang, name)
    local p = { pos = pos, ang = ang, vel = Vector(0, 0, 0), name = name }
    function p:IsValid() return true end
    function p:GetPos() return self.pos end
    function p:SetPos(x) self.pos = Vector(x) end
    function p:GetAngles() return self.ang end
    function p:SetAngles(a) self.ang = Angle(a.p, a.y, a.r) end
    function p:GetVelocity() return self.vel end
    function p:SetVelocity(v) self.vel = Vector(v) end
    function p:GetAngleVelocity() return Vector(0, 0, 0) end
    function p:AddAngleVelocity() end
    function p:Wake() end
    function p:EnableMotion() end
    function p:IsCollisionEnabled() return true end
    function p:GetMass() return 5 end
    function p:LocalToWorld(x) return LocalToWorld(x, Angle(), self.pos, self.ang) end
    function p:GetAABB() return Vector(-3, -3, -3), Vector(3, 3, 3) end
    -- (follows exactly: the target is where it goes)
    function p:ComputeShadowControl(t)
        SHADOW_CALLS = SHADOW_CALLS + 1
        self.vel = (t.pos - self.pos) * (1 / t.deltatime)
        self.pos = Vector(t.pos)
        self.ang = Angle(t.angle.p, t.angle.y, t.angle.r)
    end
    return p
end

ents = {
    Create = function(class) if class == "prop_ragdoll" then return ragdoll(nil) end end,
    FindInSphere = function() return {} end,
}

-- damage
function DamageInfo()
    local d = { damage = 0, type = 0, pos = Vector(0, 0, 0), force = Vector(0, 0, 0), attacker = NULL, inflictor = NULL }
    function d:GetDamage() return self.damage end
    function d:SetDamage(x) self.damage = x end
    function d:GetDamageType() return self.type end
    function d:SetDamageType(x) self.type = x end
    function d:IsDamageType(m) return bit.band(self.type, m) ~= 0 end
    function d:IsBulletDamage() return bit.band(self.type, DMG_BULLET) ~= 0 end
    function d:GetDamagePosition() return self.pos end
    function d:SetDamagePosition(x) self.pos = x end
    function d:GetDamageForce() return self.force end
    function d:SetDamageForce(x) self.force = x end
    function d:GetAttacker() return self.attacker end
    function d:SetAttacker(x) self.attacker = x end
    function d:GetInflictor() return self.inflictor end
    function d:SetInflictor(x) self.inflictor = x end
    return d
end

function TICK(n)
    for _ = 1, n or 1 do
        ADVANCE(1 / 66)
        hook.t.Tick.Delirium()
        RUN_TIMERS()
    end
end
