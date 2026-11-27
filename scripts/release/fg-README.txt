fg\: dlssg-to-fsr3, for frame generation on NVIDIA RTX 20/30 and GTX 16 cards
==============================================================================

dlssg-to-fsr3 by Nukem9 runs AMD FSR 3 frame generation behind the calls a game makes to
NVIDIA DLSS Frame Generation (DLSS-G). VIctory S uses it on RTX 20/30 and GTX 16 cards, which
NVIDIA's own DLSS-G does not support; on RTX 40/50 NVIDIA's own is used and these files are not.
Frame generation is off by default: Settings, "Frame generation (FSR 3)".

Licence: GPL-3.0 (LICENSE.md here). Source: https://github.com/Nukem9/dlssg-to-fsr3
These binaries were built from that source, unmodified, at commit 0e253f63
("Update readme and version"):

- dlssg_to_fsr3_amd_is_better.dll   source\maindll, preset final-universal
- dbghelp.dll                       source\wrapper_generic (preset final-universal, the
                                    "universal" loader, renamed dbghelp.dll as the project's
                                    releases do)
- dlssg_to_fsr3.ini                 resources\dlssg_to_fsr3.ini with EnableDebugOverlay=0 and
                                    EnableInterpolatedFramesOnly=0

How they were built (Windows, Visual Studio 2022 Build Tools with MSVC 19.44, CMake 3.31 and
Ninja 1.12 from the Build Tools, vcpkg, the Vulkan SDK; from a "x64 Native Tools" prompt with
VCPKG_ROOT and VULKAN_SDK set):

  git clone https://github.com/Nukem9/dlssg-to-fsr3
  cd dlssg-to-fsr3
  git checkout 0e253f63
  cmake --preset final-universal
  rem FidelityFX SDK backends (frame interpolation and optical flow), DX12 then Vulkan:
  cd dependencies\FidelityFX-SDK\sdk && mkdir build && cd build
  cmake .. -G "Visual Studio 17 2022" -A x64 -DFFX_API_BACKEND=DX12_X64 -DFFX_FI=ON -DFFX_OF=ON -DFFX_AUTO_COMPILE_SHADERS=1
  cmake --build . --config Release
  (delete CMakeCache.txt and CMakeFiles, then the same with -DFFX_API_BACKEND=VK_X64 and
   -DVulkan_LIBRARY=<repository>\bin\final-universal\vcpkg_installed\x64-windows-static\lib\vulkan-1.lib)
  cd <repository>
  cmake --build --preset final-universal-release

Outputs: bin\final-universal\source\maindll\Release\dlssg_to_fsr3_amd_is_better.dll and
bin\final-universal\source\wrapper_generic\Release\nvngx.dll (copied as dbghelp.dll).

How VIctory S uses them: before a game with frame generation on such a card, the launcher copies
dbghelp.dll, dlssg_to_fsr3_amd_is_better.dll and dlssg_to_fsr3.ini next to engine\victory.exe and
into %LOCALAPPDATA%\Sagouindupm\framegen (where NVIDIA's Streamline files go; Streamline's
sl.interposer.dll imports dbghelp.dll, which loads the mod), and takes them away again for a game
without frame generation. NVIDIA's files are never in this package: the launcher fetches them from
NVIDIA's own release when frame generation is turned on.
