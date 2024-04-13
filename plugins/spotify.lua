local bot = require("bot")
local spotify = require("spotify")

local plugin = bot.plugin("Spotify")

local client = spotify.Client:new(
   "c021ca2ee0c943e1835fdbef8b89b1cd",
   "--SECRET--"
)

plugin.cache:load()

local refresh_token = plugin.cache:get("refresh_token")
local access_token = plugin.cache:get("access_token")
local expires_at = plugin.cache:get("expires_at")

if access_token ~= nil and expires_at > os.time() then
   client.refresh_token = refresh_token
   client.access_token = access_token
   client.expires_at = expires_at
else
   print("https://accounts.spotify.com/authorize?response_type=code&client_id=c021ca2ee0c943e1835fdbef8b89b1cd&scope=user-read-private+user-read-email+user-modify-playback-state&redirect_uri=https://weirdhorse.party/callback")
   -- get code
   -- put it below
   local code = io.read()
   client:auth(code)

   plugin.cache:set("refresh_token", client.refresh_token)
   plugin.cache:set("access_token", client.access_token)
   plugin.cache:set("expires_at", client.expires_at)
   plugin.cache:save()
end

function plugin:tick(ctx)
   if os.time() > client.expires_at - 60 * 5 then
      client:auth_refresh()
      plugin.cache:set("refresh_token", client.refresh_token)
      plugin.cache:set("access_token", client.access_token)
      plugin.cache:set("expires_at", client.expires_at)
      print("Refreshed spotify token")
      plugin.cache:save()
   end
end

plugin:command(".*queue up\\s+(.*)", function(msg, matches)
  local query = matches[2]
  local r = client:search(query)
  client:enqueue(r.uri)

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
