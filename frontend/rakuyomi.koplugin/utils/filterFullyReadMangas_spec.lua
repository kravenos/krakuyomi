---@diagnostic disable: undefined-global
local filterFullyReadMangas = require('utils/filterFullyReadMangas')

describe('filterFullyReadMangas', function()
  local unread = { id = 'unread', unread_chapters_count = 2, total_chapters_count = 3, read_chapters_count = 1 }
  local fully_read = { id = 'read', unread_chapters_count = 0, total_chapters_count = 3, read_chapters_count = 3 }
  local read_gap = { id = 'gap', unread_chapters_count = 0, total_chapters_count = 3, read_chapters_count = 1 }
  local empty = { id = 'empty', unread_chapters_count = 0, total_chapters_count = 0, read_chapters_count = 0 }
  local unknown = { id = 'unknown' }
  local mangas = { unread, fully_read, read_gap, empty, unknown }

  it('keeps the library unchanged when the setting is disabled', function()
    assert.equal(mangas, filterFullyReadMangas(mangas, false, false))
  end)

  it('hides only manga known to be fully read in the main library', function()
    local filtered = filterFullyReadMangas(mangas, true, false)

    assert.same({ unread, read_gap, empty, unknown }, filtered)
  end)

  it('keeps playlists unchanged when the setting is enabled', function()
    assert.equal(mangas, filterFullyReadMangas(mangas, true, true))
  end)
end)
