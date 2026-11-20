# Runs a Lua script against the real gmsv_delirium module, inside Garry's Mod's own LuaJIT (lua_shared.dll), without
# starting the game: the module is loaded and opened as Garry's Mod does (gmod13_open), then the script runs.
# usage: python host.py script.lua [more.lua ...]   (run in order in one Lua state; DELIRIUM_DLL picks another module)
import ctypes
import os
import sys

GMOD = r"J:\SteamLibrary\steamapps\common\GarrysMod"
DLL = os.environ.get("DELIRIUM_DLL", r"J:\rust\target\delirium\release\gmsv_delirium.dll")

for d in (GMOD + r"\garrysmod\bin\win64", GMOD + r"\bin\win64"):
    if os.path.isdir(d):
        os.add_dll_directory(d)

lua = ctypes.CDLL(GMOD + r"\garrysmod\bin\win64\lua_shared.dll")
lua.luaL_newstate.restype = ctypes.c_void_p
lua.luaL_openlibs.argtypes = [ctypes.c_void_p]
lua.luaL_loadbuffer.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_size_t, ctypes.c_char_p]
lua.lua_pcall.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_int, ctypes.c_int]
lua.lua_tolstring.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p]
lua.lua_tolstring.restype = ctypes.c_char_p
lua.lua_close.argtypes = [ctypes.c_void_p]
lua.lua_getfield.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_char_p]
GLOBALS = -10002

L = lua.luaL_newstate()
lua.luaL_openlibs(L)

# Garry's Mod's Lua has no loadfile / loadstring / dofile (and can't call back into Python): every addon's Lua file is
# compiled here, into the global table SOURCES ["addon/lua/path.lua"] = its chunk, for the stand-in's include().
lua.lua_createtable.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_int]
lua.lua_setfield.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_char_p]
lua.lua_settop.argtypes = [ctypes.c_void_p, ctypes.c_int]
ADDONS = os.path.join(GMOD, "garrysmod", "addons")
lua.lua_createtable(L, 0, 64)
for addon in ("delirium", "delirium_zcity"):
    root = os.path.join(ADDONS, addon, "lua")
    for dirpath, _, files in os.walk(root):
        for f in files:
            if not f.endswith(".lua"):
                continue
            full = os.path.join(dirpath, f)
            rel = os.path.relpath(full, root).replace("\\", "/")
            src = open(full, "rb").read()
            if lua.luaL_loadbuffer(L, src, len(src), ("@" + addon + "/" + rel).encode()) != 0:
                print("LUA SYNTAX ERROR:", (lua.lua_tolstring(L, -1, None) or b"?").decode(errors="replace"), flush=True)
                lua.lua_settop(L, -2)
                continue
            lua.lua_setfield(L, -2, (addon + "/" + rel).encode())
lua.lua_setfield(L, GLOBALS, b"SOURCES")
mod = ctypes.CDLL(DLL)
mod.gmod13_open.argtypes = [ctypes.c_void_p]
mod.gmod13_close.argtypes = [ctypes.c_void_p]
mod.gmod13_open(L)

failed = False
for script in sys.argv[1:]:
    src = open(script, "rb").read()
    failed = lua.luaL_loadbuffer(L, src, len(src), ("@" + script).encode()) != 0 or lua.lua_pcall(L, 0, 0, 0) != 0
    if failed:
        print("LUA ERROR:", (lua.lua_tolstring(L, -1, None) or b"?").decode(errors="replace"), flush=True)
        break
# (Garry's Mod's print goes to the game's console, which isn't there: scripts leave what they say in RESULT)
lua.lua_getfield(L, GLOBALS, b"RESULT")
print((lua.lua_tolstring(L, -1, None) or b"").decode(errors="replace"), flush=True)
mod.gmod13_close(L)
lua.lua_close(L)
sys.exit(1 if failed else 0)
