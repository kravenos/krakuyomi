--- Filters fully read manga from the main library without changing playlists.
--- Manga with an unknown unread count remain visible.
--- @param mangas Manga[]
--- @param enabled boolean
--- @param is_playlist boolean
--- @return Manga[]
return function(mangas, enabled, is_playlist)
  if not enabled or is_playlist then
    return mangas
  end

  local filtered = {}
  for _, manga in ipairs(mangas) do
    if manga.unread_chapters_count == nil or manga.unread_chapters_count > 0 then
      table.insert(filtered, manga)
    end
  end

  return filtered
end
