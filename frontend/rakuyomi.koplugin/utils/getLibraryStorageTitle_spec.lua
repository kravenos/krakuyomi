---@diagnostic disable: undefined-global
local getLibraryStorageTitle = require('utils/getLibraryStorageTitle')

describe('getLibraryStorageTitle', function()
  local format_bytes = function(bytes)
    return bytes .. ' bytes'
  end

  it('formats a positive downloaded total', function()
    local title = getLibraryStorageTitle(
      { type = 'SUCCESS', body = { total_bytes = 2048 } },
      'Total downloaded',
      format_bytes
    )

    assert.equal('Total downloaded: 2048 bytes', title)
  end)

  it('hides zero totals', function()
    local title = getLibraryStorageTitle(
      { type = 'SUCCESS', body = { total_bytes = 0 } },
      'Total downloaded',
      format_bytes
    )

    assert.is_nil(title)
  end)

  it('hides failed or malformed responses', function()
    assert.is_nil(getLibraryStorageTitle({ type = 'ERROR' }, 'Total downloaded', format_bytes))
    assert.is_nil(getLibraryStorageTitle({ type = 'SUCCESS', body = {} }, 'Total downloaded', format_bytes))
    assert.is_nil(getLibraryStorageTitle(nil, 'Total downloaded', format_bytes))
  end)
end)
