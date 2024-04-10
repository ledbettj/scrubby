local Plugin = Bot.plugin("Test")
local http = require("http")
local json = require("json")

local _zones = json.decode(http.get("https://www.timeapi.io/api/TimeZone/AvailableTimeZones"))
local zones = {}
for i, v in ipairs(_zones) do
   zones[v] = true
   print(v)
end

Bot.command("what time is it in ([^?]+)", function(msg, matches)
  local where = matches[2]
  local res = nil

  if zones[where] then
     local resp = json.decode(http.get("https://www.timeapi.io/api/TimeZone/zone?timeZone=" .. where))
     res = resp["currentLocalTime"]
  else
     res = 'Scrubby time'
  end

  return "It's " .. res .. " in " .. where .. ", " .. msg.author .. "."
end)

Bot.register(Plugin)
