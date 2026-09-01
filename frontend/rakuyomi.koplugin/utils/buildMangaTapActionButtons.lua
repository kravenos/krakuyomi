--- Builds the two actions shown when a manga is tapped.
--- @param continue_text string
--- @param chapter_list_text string
--- @param continue_callback fun(): nil
--- @param chapter_list_callback fun(): nil
--- @return table
return function(continue_text, chapter_list_text, continue_callback, chapter_list_callback)
  return {
    { {
      text = continue_text,
      callback = continue_callback,
    } },
    { {
      text = chapter_list_text,
      callback = chapter_list_callback,
    } },
  }
end
