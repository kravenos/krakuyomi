--- Selects the direct library return callback, with a safe chapter-list fallback.
--- @param library_callback nil|fun(): nil
--- @param chapter_list_callback fun(): nil
--- @return fun(): nil
return function(library_callback, chapter_list_callback)
  return library_callback or chapter_list_callback
end
