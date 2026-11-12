"""Offline comparisons with preserved source preparation; no native imports."""

import ast
import copy
import hashlib
from pathlib import Path
import struct
import sys
import tempfile
import types
import unittest
from unittest.mock import patch
import zipfile

import check_geometry as checked

ROOT = Path(__file__).resolve().parents[3]


class GeometryPreparationTests(unittest.TestCase):
    def evidence(self, *relative_paths):
        paths = [ROOT / relative for relative in relative_paths]
        missing = [str(path) for path in paths if not path.exists()]
        if missing:
            self.skipTest('External retained evidence unavailable: ' + ', '.join(missing))
        return paths

    def inputs(self):
        import json
        return json.loads(checked.pinned(
            ROOT / 'tools/marker4_v8/paired/inputs.json', checked.INPUT_SHA))

    def test_historical_preparation_matches_preserved_expression(self):
        self.evidence('tools/marker4_v8/paired/inputs.json',
                      'tools/marker4_v8/paired/geometry.py')
        inputs = self.inputs()
        path = ROOT / 'tools/marker4_v8/paired/geometry.py'
        tree = ast.parse(checked.pinned(path,
            'fee195c3167411f454a5ceba5b5517624ebd4a9f7a0b270f7c57a2ee735cde66'))
        expression = next(node.value for node in tree.body
                          if isinstance(node, ast.Assign)
                          and any(isinstance(t, ast.Name) and t.id == 'raw' for t in node.targets))
        original = eval(compile(ast.Expression(expression), str(path), 'eval'),
                        {'s': inputs['source']})
        source = inputs['source']
        self.assertEqual(original, checked.source_geometry(
            source, pressures=source['pressures'], pen_width=source['pen_width_f32']))

    def test_current_pressure_matches_preserved_rounding_and_getter(self):
        self.evidence('hf/04-marker4-highlighter.sdocx',
                      'tools/marker4_v8/paired/current_geometry.py')
        archive_path = ROOT / 'hf/04-marker4-highlighter.sdocx'
        checked.pinned(archive_path, checked.ARCHIVE_SHA)
        with zipfile.ZipFile(archive_path) as archive:
            page = archive.read(checked.PAGE_NAME)
        # Execute only the preserved decode assignments/loop, never prefix exec,
        # archive access, imports, or the native invocation.
        path = ROOT / 'tools/marker4_v8/paired/current_geometry.py'
        tree = ast.parse(checked.pinned(path,
            '464a54fd2b6d8627d52d88096ba9d5f9a0ca90aeb91613132aacd8362579ebed'))
        start = next(i for i, node in enumerate(tree.body)
                     if isinstance(node, ast.Assign)
                     and any(isinstance(t, ast.Name) and t.id == 'wire' for t in node.targets))
        end = next(i for i, node in enumerate(tree.body)
                   if isinstance(node, ast.Assign)
                   and any(isinstance(t, ast.Subscript) for t in node.targets))
        scope = {'page': page, 'struct': struct}
        exec(compile(ast.Module(tree.body[start:end], type_ignores=[]), str(path), 'exec'), scope)
        actual = checked.decode_saved_pressures(page)
        self.assertEqual(struct.pack('<253f', *actual), struct.pack('<253f', *scope['pressure']))
        self.assertEqual(hashlib.sha256(struct.pack('<253f', *actual)).hexdigest(), checked.PRESSURE_SHA)
        with self.assertRaisesRegex(ValueError, 'page changed'):
            checked.decode_saved_pressures(page[:-1])

    def test_exact_support_binding_and_import_state_restoration(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            support = directory / 'renamed_support.py'
            support.write_text('VALUE = "pinned"\n')
            (directory / 'marker4_native.py').write_text('raise RuntimeError("wrong support")\n')
            runner = directory / 'runner.py'
            runner.write_text('import marker4_native\nVALUE = marker4_native.VALUE\n')
            old_path = sys.path[:]
            old_support = sys.modules.get('marker4_native')
            old_runner = sys.modules.get('native_v8')
            with checked.native_module(runner, support, directory) as module:
                self.assertEqual(module.VALUE, 'pinned')
                self.assertEqual(Path(sys.modules['marker4_native'].__file__), support)
            self.assertEqual(sys.path, old_path)
            self.assertIs(sys.modules.get('marker4_native'), old_support)
            self.assertIs(sys.modules.get('native_v8'), old_runner)

    def test_both_sizes_and_independent_comparison_failures(self):
        source = {
            'profile_name_resolved': 'Marker4', 'advanced_settings_resolved': '8;',
            'tool_type_raw': 2, 'points': [[1.25, 2.5], [3.5, 4.75]],
            'pressures': [0.25, 0.75], 'timestamps': [10, 20],
            'tilts': [0.5, 0.5], 'orientations': [1.0, 1.0],
        }
        expected = {'dots': [[1.25, 2.5, 0.0]], 'sample_ends': [0, 1]}
        inputs = {'source': source, 'native': {'native_dots': expected['dots'],
                                              'sample_ends': expected['sample_ends']}}
        pressures = [0.125, 0.875]
        calls = []
        def oracle(raw):
            calls.append(raw)
            return copy.deepcopy(expected)
        checked.check_geometry(inputs, pressures, oracle, current_pressures=True)
        self.assertEqual([raw['pen_width'] for raw in calls], [0.4, 2.0])
        for raw in calls:
            self.assertEqual(raw['pressures'], pressures)
            self.assertEqual(raw['points'], [{'x': 1.25, 'y': 2.5}, {'x': 3.5, 'y': 4.75}])
            self.assertEqual(raw['rendering']['tool_type_raw'], 2)
        for field, message in [('dots', 'dots differ'), ('sample_ends', 'boundaries differ')]:
            bad = copy.deepcopy(expected)
            bad[field] = []
            with self.assertRaisesRegex(ValueError, message):
                checked.check_geometry(inputs, pressures, lambda raw: bad, current_pressures=False)
        results = iter([{**expected, 'extra': 1}, {**expected, 'extra': 2}])
        with self.assertRaisesRegex(ValueError, 'full outputs differ'):
            checked.check_geometry(inputs, pressures, lambda raw: next(results), current_pressures=True)

    def test_import_bindings_restore_absent_none_and_module_on_success_and_failure(self):
        names = ('marker4_native', 'native_v8')
        existing = types.ModuleType('existing_binding')
        for initial in ('absent', None, existing):
            for failing in (False, True):
                with self.subTest(initial=initial, failing=failing), tempfile.TemporaryDirectory() as temporary:
                    directory = Path(temporary)
                    support = directory / 'support.py'
                    support.write_text('VALUE = "bound"\n')
                    runner = directory / 'runner.py'
                    runner.write_text('import marker4_native\nVALUE = marker4_native.VALUE\n'
                                      + ('raise RuntimeError("runner failed")\n' if failing else ''))
                    with patch.dict(sys.modules):
                        for name in names:
                            if initial == 'absent':
                                sys.modules.pop(name, None)
                            else:
                                sys.modules[name] = initial
                        old_path = sys.path[:]
                        if failing:
                            with self.assertRaisesRegex(RuntimeError, 'runner failed'):
                                with checked.native_module(runner, support, directory):
                                    self.fail('failing import yielded a runner')
                        else:
                            with checked.native_module(runner, support, directory) as module:
                                self.assertEqual(module.VALUE, 'bound')
                        self.assertEqual(sys.path, old_path)
                        for name in names:
                            if initial == 'absent':
                                self.assertNotIn(name, sys.modules)
                            else:
                                self.assertIn(name, sys.modules)
                                self.assertIs(sys.modules[name], initial)


if __name__ == '__main__':
    unittest.main()
