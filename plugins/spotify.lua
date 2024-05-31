local bot = require("bot")
local spotify = require("spotify")
local json = require("json")

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

plugin:command({
      name = "enqueue",
      description = [[
  Add a song to the list of songs to be played. Returns information about the song that was added, including the album cover image.
]],
      schema = {
         ["type"] = "object",
         properties = {
            artist = {
               ["type"] = "string",
               description = "the name of a musical artist",
            },
            track = {
               ["type"] = "string",
               description = "the title of a song",
            },
         },
         required = { "song" }
      },
      method = function(self, params)
         self:log("Searching for ", json.encode(params))

         local r = client:search(params)
         client:enqueue(r.uri)

         return json.encode({
               instructions = "When returning this data to the User, use the JSON format.",
               title = r.name,
               thumbnail = r.album.images[1].url,
               ["Artist"] = r.artists[1].name,
               ["Album"] = r.album.name
         })
      end
})

plugin:command({
      name = "up_next",
      description = [[
        Tells you what song is coming up next in the list of songs to play. Returns information about the song, including the album cover image
      ]],
      schema = nil,
      method = function(self, params)
         local r = client:list_queue()
         local q = r.queue[1]

         if q ~= nil then
            return json.encode({
               instructions = "When returning this data to the User, use the JSON format.",
               title = q.name,
               thumbnail = q.album.images[1].url,
               ["Artist"] = q.artists[1].name,
               ["Album"] = q.album.name,
            })
         else
            return "There is nothing in the queue to play"
         end
      end
})


bot:register(plugin)
