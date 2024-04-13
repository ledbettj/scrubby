local http = require('http')
local json = require('json')
local b64 = require('base64')

local API_URL = "https://api.spotify.com/v1"
local ACCOUNTS_URL = "https://accounts.spotify.com/api"

local Client = {}

local function url_encode(str)
   local str = string.gsub(str, "([^%w%.%- ])", function(c)
                              return string.format("%%%02X", string.byte(c))
                           end)
   str = string.gsub(str, " ", "+")
   return str
end

function Client:new(client_id, client_secret)
   client = { expires_at = 0, client_id = client_id, client_secret = client_secret }
   setmetatable(client, self)
   self.__index = self
   return client
end

function Client:auth(code)
   local resp = http.post(
      ACCOUNTS_URL .. "/token",
      "grant_type=authorization_code&code=" .. code .. "&redirect_uri=https://weirdhorse.party/callback",
      {
         headers = {
            ["Content-Type"] = "application/x-www-form-urlencoded",
            ["Authorization"] = "Basic " .. b64.encode(self.client_id .. ":" .. self.client_secret)
         }
   })
   local data = json.decode(resp)

   self.refresh_token = data.refresh_token
   self.access_token = data.access_token
   self.expires_at = os.time() + data.expires_in
end

function Client:auth_refresh()
   local resp = http.post(
      ACCOUNTS_URL .. "/token",
      "grant_type=refresh_token&refresh_token=" .. self.refresh_token,
      {
         headers = {
            ["Content-Type"] = "application/x-www-form-urlencoded",
            ["Authorization"] = "Basic " .. b64.encode(self.client_id .. ":" .. self.client_secret)
         }
   })
   local data = json.decode(resp)

   self.access_token = data.access_token
   self.refresh_token = data.refresh_token
   self.expires_at = os.time() + data.expires_in
end

function Client:search(query)
   local resp = http.get(
      API_URL .. "/search?type=track&limit=1&q=" .. url_encode(query),
      { headers = { ['Authorization'] = "Bearer " .. self.access_token } }
   )
   local data = json.decode(resp)

   return data.tracks.items[1]
end

function Client:enqueue(track_id)
   http.post(
      API_URL .. "/me/player/queue?uri=" .. url_encode(track_id),
      "",
      { headers = { ['Authorization'] = "Bearer " .. self.access_token } }
   )
end

function Client:list_queue()
   local resp = http.get(
      API_URL .. "/me/player/queue",
      { headers = { ['Authorization'] = "Bearer " .. self.access_token } }
   )
   return json.decode(resp)
end

return { Client = Client }
