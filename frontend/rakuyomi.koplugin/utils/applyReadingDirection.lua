--- Applies an explicit reading direction to a KOReader view when needed.
--- @param view table|nil
--- @param direction string|nil
--- @return boolean changed
return function(view, direction)
  if view == nil or type(view.onToggleReadingOrder) ~= "function" then
    return false
  end
  if direction ~= "ltr" and direction ~= "rtl" then
    return false
  end

  local want_inverse = direction == "rtl"
  if (view.inverse_reading_order == true) == want_inverse then
    return false
  end

  view:onToggleReadingOrder(want_inverse)
  return true
end
