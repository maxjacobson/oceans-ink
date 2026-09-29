project(
  'oceans-ink',
  version: '0.1.0',
  license: 'MIT',
  meson_version: '>= 1.2',
)

gnome = import('gnome')

gnome.compile_resources(
  'oceans-ink',
  'src/resources/resources.gresource.xml',
  source_dir: 'src/resources',
  gresource_bundle: true,
  install: true,
  install_dir: get_option('datadir') / 'oceans-ink',
)

cargo_sh = find_program('build-aux/cargo.sh')

custom_target(
  'oceans-ink-bin',
  build_always_stale: true,
  output: 'oceans-ink',
  install: true,
  install_dir: get_option('bindir'),
  command: [cargo_sh, meson.project_source_root(), meson.project_build_root(), '@OUTPUT@'],
)

subdir('data')
