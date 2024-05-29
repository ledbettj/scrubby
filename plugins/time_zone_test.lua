local bot = require("bot")
local http = require("http")
local json = require("json")

local plugin = bot.plugin("Time Test")
local cache = plugin.cache

cache:load()

local zones = cache:get("zones")
if not zones then
   zones = {}
   local raw_zones = json.decode(http.get("https://www.timeapi.io/api/TimeZone/AvailableTimeZones").body)
   for i, v in ipairs(raw_zones) do
      zones[v] = true
   end
   cache:set("zones", zones)
   cache:save()
end

plugin:command("what time is it in ([^?]+)", function(self, msg, matches)
  local where = matches[2]
  local res = nil

  if zones[where] then
     local resp = json.decode(http.get("https://www.timeapi.io/api/TimeZone/zone?timeZone=" .. where).body)
     res = resp["currentLocalTime"]
  else
     res = 'Scrubby time'
  end

  return "It's " .. res .. " in " .. where .. ", " .. msg.author .. "."
end)

bot:register(plugin)
