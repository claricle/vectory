# Windows CI Known Issues

## Two Issues Identified

### 1. Path Separator Issue (FIXED)

**Problem:** Ruby's `File.join` uses forward slashes (`/`) on all platforms, including Windows. This creates paths with mixed separators:
```
C:\Program Files\Inkscape\bin/inkscape.EXE
```

**Solution:** Fixed in ukiryu PR: https://github.com/ukiryu/ukiryu/pull/6

**Status:** ✅ Fixed - executable paths now show all backslashes: `C:\Program Files\Inkscape\bin\inkscape.EXE`

### 2. Inkscape 1.4.2 PDF Import Issue

**Problem:** Inkscape 1.4.2 on Windows returns exit code 0 but doesn't create output files when importing PDF.

**Solution:** Inkscape 1.4.3 has been released that fixes this issue.

**Status:** ⏳ Pending - CI currently installs Inkscape 1.4.2. Need to update CI to use 1.4.3.

## Current Test Results

| Platform | Status |
|----------|--------|
| macOS | ✅ Pass |
| Ubuntu | ✅ Pass |
| Windows | ❌ 28 failures (Inkscape 1.4.2 PDF import issue) |

## Next Steps

1. Merge ukiryu PR #6 (path separator fix)
2. Update CI to install Inkscape 1.4.3 (or later)
3. Remove test skips once Inkscape is updated

