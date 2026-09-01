--- Filters fully read manga from the main library without changing playlists.
--- Manga with unknown counts, no cached chapters, or read gaps remain visible.
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
    local total = manga.total_chapters_count
    local read = manga.read_chapters_count
    local fully_read = total ~= nil and total > 0 and read ~= nil and read >= total
    if not fully_read then
      table.insert(filtered, manga)
    end
  end

  return filtered
end
