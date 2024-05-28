local bot = require("bot")
local spotify = require("spotify")

local plugin = bot.plugin("Spotify")

local client = spotify.Client:new("c021ca2ee0c943e1835fdbef8b89b1cd", env.SPOTIFY_SECRET)

plugin.cache:load()

local refresh_token = plugin.cache:get("refresh_token")
local access_token = plugin.cache:get("access_token")
local expires_at = plugin.cache:get("expires_at")

local function save_state(client)
   plugin.cache:set("refresh_token", client.refresh_token)
   plugin.cache:set("access_token", client.access_token)
   plugin.cache:set("expires_at", client.expires_at)
   plugin.cache:save()
end

if refresh_token ~= nil then
   client.refresh_token = refresh_token
   client.access_token = access_token
   client.expires_at = expires_at
else
   print("https://accounts.spotify.com/authorize?response_type=code&client_id=c021ca2ee0c943e1835fdbef8b89b1cd&scope=user-read-private+user-read-email+user-modify-playback-state+user-read-playback-state+user-read-currently-playing&redirect_uri=https://weirdhorse.party/callback")
   -- get code
   -- put it below
   local code = io.read()
   client:auth(code)
   save_state(client)
end

function plugin:tick(ctx)
   if os.time() > client.expires_at - 60 * 5 then
      client:auth_refresh()
      save_state(client)
      self:log("Refreshed spotify token")
   end
end

plugin:command(
   ".*queue up\\s+(.*)",
   function(self, msg, matches)
      local query = matches[2]
      self:log("Searching for ", query)

      local r = client:search(query)
      client:enqueue(r.uri)

      return {
         content = "Queued up!",
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

plugin:command(
   "what('?)s up next",
   function(self, msg, matches)
      local r = client:list_queue()
      local q = r.queue[1]

      if q ~= nil then
         return {
            content = "Next Track:",
            embed = {
               title = q.name,
               thumbnail = q.album.images[1].url,
               fields = {
                  { "Artist", q.artists[1].name, true },
                  { "Album", q.album.name, true }
               }
            }
         }
      else
         return "Nothing queued up! Tell me 'queue up ...' to add a song to the queue."
      end
end)


bot:register(plugin)
