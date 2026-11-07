Patches for files owned by someone else while the cross-platform port was written. Apply them
once the UI/render work has landed, then delete this folder.

render-text-fonts.diff          src/render/text.rs: find fonts on macOS and Linux, not just C:\Windows\Fonts
                                  (git apply packaging/patches/render-text-fonts.diff)
render-custom-themes-dir.diff   src/render/theme/custom.rs: themes folder from settings::config_dir()
                                  (two lines: apply by hand)
