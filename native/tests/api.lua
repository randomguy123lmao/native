-- The module's Lua API, end to end, on this machine's GTA IV. Run: python host.py api.lua
local failed = 0
local clock = (os and os.clock) or function() return 0 end
RESULT = ""
local function out(s) RESULT = RESULT .. s .. "\n" end
local function check(name, ok, detail)
    if not ok then failed = failed + 1 end
    out((ok and "PASS  " or "FAIL  ") .. name .. (detail and ("   [" .. tostring(detail) .. "]") or ""))
end
local GTA = [[J:\SteamLibrary\steamapps\common\Grand Theft Auto IV\GTAIV]]

check("the module opened as Garry's Mod opens it", type(delirium) == "table" and delirium.Version ~= nil)
local ok, err = delirium.Message(1, "bodyBalance", {})
check("calls before Init say so (no Lua error)", ok == nil and type(err) == "string", err)

local t0 = clock()
ok, err = delirium.Init(GTA)
check("Init with the GTA IV folder", ok == true, err or string.format("%.2f s", clock() - t0))
local info = delirium.Info()
check("21 parts, the game's surfaces", info.parts == 21 and info.surfaces > 100, info.parts .. " parts, " .. info.surfaces .. " surfaces")
local names = delirium.PartNames()
check("part names (the engine's)", #names == 21 and names[1] == "Buttocks" and names[21] == "Hand_Right", names[1] .. " .. " .. names[21])

-- a floor: 40 m square at z = 0
local n = delirium.MeshAdd(1, {-20, -20, 0, 20, -20, 0, 20, 20, 0, -20, 20, 0}, {0, 1, 2, 0, 2, 3}, 0)
check("a mesh added", n == 2, n)

-- facing +X (yaw 0): the left hip is towards +Y
local id = delirium.Create("", 0, 0, 0, 0)
check("a character made", type(id) == "number", id)
local p = delirium.Parts(id)
check("standing on the floor", #p == 147 and p[3] > 0.8 and p[3] < 1.1, "pelvis z " .. p[3])
check("facing yaw 0 (+X): left hip at +Y", p[1 * 7 + 2] > p[4 * 7 + 2], string.format("left y %.2f right y %.2f", p[1 * 7 + 2], p[4 * 7 + 2]))

local unused = delirium.Message(id, "bodyBalance", {start = true, bodyStiffness = 10})
check("bodyBalance started", unused == 0, unused)
local a, why = delirium.Message(id, "noSuchMessage", {})
check("an unknown message is refused", a == nil and why:find("no message"), why)
a, why = delirium.Message(id, "bodyBalance", {nope = 1})
check("an unknown parameter is refused", a == nil and why:find("no parameter"), why)
a, why = delirium.Message(id, "headLook", {start = true, pos = {1, 2, 1.6}})
check("a vector parameter (table of 3)", a ~= nil, why)
a, why = delirium.Message(id, "headLook", {pos = {x = 2, y = 1, z = 1.6}})
check("a vector parameter (x, y, z fields)", a ~= nil, why)

t0 = clock()
for i = 1, 60 do delirium.Step(1 / 60) end
local st = delirium.State(id)
check("balancing after a second", st.balancing and not st.fallen, string.format("%.3f ms a step", (clock() - t0) * 1000 / 60))

p = delirium.Parts(id)
local cx, cy, cz = p[10 * 7 + 1], p[10 * 7 + 2], p[10 * 7 + 3]
delirium.Push(id, cx, cy, cz, -60, 0, 0)
delirium.Shot(id, "POWERFUL_WEAPON", 10, cx, cy, cz, -1, 0, 0, 200)
for i = 1, 90 do delirium.Step(1 / 60) end
local p2 = delirium.Parts(id)
check("shot from the front: pushed back (-X)", p2[1] < p[1] - 0.1, string.format("pelvis x %.2f -> %.2f", p[1], p2[1]))
local fb = delirium.Feedback(id)
check("feedback is a list", type(fb) == "table", #fb .. " entries")

-- a big push: it falls
local id2 = delirium.Create("", 3, 0, 0, math.pi / 2)
delirium.Message(id2, "bodyBalance", {start = true})
for i = 1, 30 do delirium.Step(1 / 60) end
p = delirium.Parts(id2)
delirium.Push(id2, p[10 * 7 + 1], p[10 * 7 + 2], p[10 * 7 + 3], 0, 400, 0)
for i = 1, 180 do delirium.Step(1 / 60) end
st = delirium.State(id2)
p = delirium.Parts(id2)
check("a big push knocks the second one down", st.fallen or p[3] < 0.5, "pelvis z " .. string.format("%.2f", p[3]) .. ", fallen " .. tostring(st.fallen))

-- placed in a pose (a handoff): every part where we put it
local pose = {}
local now = delirium.Parts(id)
for k = 0, 20 do
    for j = 1, 7 do pose[#pose + 1] = now[k * 7 + j] + (j == 1 and 1 or 0) end
    for j = 1, 6 do pose[#pose + 1] = 0 end
end
check("SetParts", delirium.SetParts(id, pose) == true)
local moved = delirium.Parts(id)
check("... every part moved there", math.abs(moved[1] - (now[1] + 1)) < 1e-3, string.format("%.3f vs %.3f", moved[1], now[1] + 1))

-- a body fitted to a taller skeleton
local len = {}
for k = 1, 21 do len[k] = 1.1 end
check("Fit a body 10% bigger", delirium.Fit("tall", len, 1.2) == true)
local rest, tall = delirium.RestPose(""), delirium.RestPose("tall")
check("... it stands taller", tall[3] > rest[3] * 1.05, string.format("pelvis %.3f vs %.3f", tall[3], rest[3]))
local id3 = delirium.Create("tall", -3, 0, 0, 0)
delirium.Message(id3, "bodyBalance", {start = true})
for i = 1, 60 do delirium.Step(1 / 60) end
check("... and balances", delirium.State(id3).balancing)

check("Destroy", delirium.Destroy(id2) == true and delirium.Count() == 2)
check("the world raycast hits the floor", delirium.WorldRaycast(0, 0, 5, 0, 0, -1, 10) ~= nil)
delirium.MeshRemove(1)
check("the mesh removed", delirium.WorldRaycast(0, 0, 5, 0, 0, -1, 10) == nil)
local msgs = delirium.Messages()
check("the message list", #msgs >= 55, #msgs .. " messages")
local ps = delirium.Params("splitBody")
check("splitBody's parameters", #ps > 20 and ps[1].name == "aimTarget", ps[1] and ps[1].name)

out(failed == 0 and "all passed" or (failed .. " FAILED"))
