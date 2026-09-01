---@diagnostic disable: undefined-global
local selectBackToLibraryCallback = require('utils/selectBackToLibraryCallback')

describe('selectBackToLibraryCallback', function()
  it('selects the saved library callback when available', function()
    local library_callback = function() end
    local chapter_list_callback = function() end

    assert.equal(library_callback, selectBackToLibraryCallback(library_callback, chapter_list_callback))
  end)

  it('falls back to the chapter list callback', function()
    local chapter_list_callback = function() end

    assert.equal(chapter_list_callback, selectBackToLibraryCallback(nil, chapter_list_callback))
  end)
end)
