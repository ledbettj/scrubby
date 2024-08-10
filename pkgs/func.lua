
function filter(tbl, func)
   local newtbl = {}
   for i,v in pairs(tbl) do
      if func(v) then
         newtbl[i] = v
      end
   end
   return newtbl
end

function map(tbl, func)
   local newtbl = {}
   for i,v in pairs(tbl) do
      newtbl[i] = func(v)
   end
   return newtbl
end

return { filter = filter, map = map }
