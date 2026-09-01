--- Builds the optional downloaded-storage title for the library menu.
--- @param response table|nil
--- @param label string
--- @param format_bytes fun(bytes: number): string
--- @return string|nil
return function(response, label, format_bytes)
  local total_bytes = type(response) == 'table'
      and response.type == 'SUCCESS'
      and response.body
      and response.body.total_bytes
  if type(total_bytes) ~= 'number' or total_bytes <= 0 then
    return nil
  end

  return label .. ": " .. format_bytes(total_bytes)
end
