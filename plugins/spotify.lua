local bot = require("bot")
local spotify = require("spotify")

local plugin = bot.plugin("Spotify")

-- spotify:init(
--    "c021ca2ee0c943e1835fdbef8b89b1cd",
--    "CLIENT_SECRET"
-- )


plugin.cache:load()


local refresh_token = plugin.cache:get("refresh_token")
local access_token = plugin.cache:get("access_token")
local expires_at = plugin.cache:get("expires_at")

if access_token ~= nil and expires_at > os.time() then
   spotify.refresh_token = refresh_token
   spotify.access_token = access_token
   spotify.expires_at = expires_at
else
   print("https://accounts.spotify.com/authorize?response_type=code&client_id=c021ca2ee0c943e1835fdbef8b89b1cd&scope=user-read-private+user-read-email+user-modify-playback-state&redirect_uri=https://weirdhorse.party/callback")
   -- get code
   -- put it below
   local code = io.read()
   spotify:auth(code)
   plugin.cache:set("refresh_token", spotify.refresh_token)
   plugin.cache:set("access_token", spotify.access_token)
   plugin.cache:set("expires_at", spotify.expires_at)
   plugin.cache:save()
end

function plugin:tick(ctx)
   if os.time() > spotify.expires_at - 60 * 5 then
      spotify:auth_refresh()
      plugin.cache:set("refresh_token", spotify.refresh_token)
      plugin.cache:set("access_token", spotify.access_token)
      plugin.cache:set("expires_at", spotify.expires_at)
      print("Refreshed spotify token")
      plugin.cache:save()
   end
end

plugin:command(".*queue up\\s+(.*)", function(msg, matches)
  local query = matches[2]
  local r = spotify:search(query)
  spotify:enqueue(r.uri)

  return {
     embed = {
        title = r.name,
        thumbnail = r.album.images[1].url,
        fields = {
           { "Artist", r.artists[1].name, true },
           { "Album", r.album.name, true }
        }
     }
  }
end)

bot:register(plugin)
