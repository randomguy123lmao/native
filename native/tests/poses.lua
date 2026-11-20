-- Which animPose angle bends the spine forward, and which way the arms go: each tried on a standing body.
local out = function(s) RESULT = (RESULT or "") .. s .. "\n" end
delirium.Init(DELIRIUM_GTA or delirium.FindGTA())
delirium.MeshAdd(1, {-20, -20, 0, 20, -20, 0, 20, 20, 0, -20, 20, 0}, {0, 1, 2, 0, 2, 3}, 0)
local J = delirium.Joints()
local function list(t) local s = {} for _, j in ipairs(t) do s[#s + 1] = tostring(j) end return table.concat(s, ",") end
out("spine " .. list(J.spine) .. " neck " .. list(J.neck) .. " shoulder " .. list(J.shoulder) .. " elbow " .. list(J.elbow) .. " knee " .. list(J.knee) .. " hip " .. list(J.hip))
-- facing +X (yaw 0): forward is +X, left +Y
local function try(label, joints, comp, angle, msk, part)
    local id = delirium.Create("", 0, 0, 0, 0)
    delirium.Message(id, "bodyBalance", { start = true })
    for i = 1, 30 do delirium.Step(1 / 60) end
    local p0 = delirium.Parts(id)
    local pose = {}
    for _, j in ipairs(joints) do
        local a = { 0, 0, 0 }
        a[comp] = angle
        pose[#pose + 1] = j .. ":" .. a[1] .. "," .. a[2] .. "," .. a[3]
    end
    local r, why = delirium.Message(id, "animPose", { start = true, pose = table.concat(pose, ";"), mask = msk, stiffness = 12 })
    for i = 1, 60 do delirium.Step(1 / 60) end
    local p = delirium.Parts(id)
    local k = part * 7
    local d = { p[k + 1] - p[1] - (p0[k + 1] - p0[1]), p[k + 2] - p[2] - (p0[k + 2] - p0[2]), p[k + 3] - p[3] - (p0[k + 3] - p0[3]) }
    out(string.format("%-28s comp %d %+.1f: part %d moved fwd %+.2f left %+.2f up %+.2f %s", label, comp, angle, part, d[1], d[2], d[3], r and "" or tostring(why)))
    delirium.Destroy(id)
end
try("right shoulder", { J.shoulder[2] }, 1, 0.8, "ur", 20)
try("right shoulder", { J.shoulder[2] }, 3, 0.8, "ur", 20)
try("right elbow", { J.elbow[2] }, 1, 1.2, "ur", 20)
try("neck", J.neck, 1, 0.5, "un", 12)
try("knees", J.knee, 1, 0.6, "lb", 0)
try("knees (minus)", J.knee, 1, -0.6, "lb", 0)
