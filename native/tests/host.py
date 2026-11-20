# Runs a Lua script against the real gmsv_delirium module, inside Garry's Mod's own LuaJIT (lua_shared.dll), without
# starting the game: the module is loaded and opened as Garry's Mod does (gmod13_open), then the script runs.
# usage: python host.py script.lua [more.lua ...]   (run in order in one Lua state; DELIRIUM_DLL picks another module)
import ctypes
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
NATIVE = os.path.dirname(HERE)


def find_gmod():
    """Garry's Mod's folder (its LuaJIT runs the tests): GMOD_DIR, else a folder above this one, else Steam's default."""
    if os.environ.get("GMOD_DIR"):
        return os.environ["GMOD_DIR"]
    d = NATIVE
    while os.path.dirname(d) != d:
        if os.path.isfile(os.path.join(d, "garrysmod", "bin", "win64", "lua_shared.dll")):
            return d
        d = os.path.dirname(d)
    return r"C:\Program Files (x86)\Steam\steamapps\common\GarrysMod"


GMOD = find_gmod()
# the module under test: DELIRIUM_DLL, else this folder's own release build (or CARGO_TARGET_DIR's)
TARGET = os.environ.get("CARGO_TARGET_DIR", os.path.join(NATIVE, "target"))
DLL = os.environ.get("DELIRIUM_DLL", os.path.join(TARGET, "release", "gmsv_delirium.dll"))

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
# the addons' Lua (delirium/, delirium_zcity/): DELIRIUM_ADDONS, else beside this folder
ADDONS = os.environ.get("DELIRIUM_ADDONS", os.path.dirname(NATIVE))
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
# (Garry's Mod's Lua has no os.getenv: DELIRIUM_GTA handed in as a global, for the scripts)
if os.environ.get("DELIRIUM_GTA"):
    lua.lua_pushstring.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
    lua.lua_pushstring(L, os.environ["DELIRIUM_GTA"].encode())
    lua.lua_setfield(L, GLOBALS, b"DELIRIUM_GTA")
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
