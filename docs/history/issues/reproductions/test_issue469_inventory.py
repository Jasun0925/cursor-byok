"""Boundary tests for the experimental metadata collector; no model or network."""

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import issue469_inventory as probe


class InventoryTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="issue469-inventory-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()

    def write(self, path, content="SOURCE_BODY_MUST_NOT_BE_TRANSFERRED"):
        target = self.root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(content)
        return target

    def test_inventory_reads_rules_but_never_opens_source_bodies(self):
        self.write("visible.py")
        self.write("nested/ignored.py")
        self.write("nested/.cursorignore", "ignored.py\n")
        opened = []
        real_open = os.open

        def record(path, flags, *args, **kwargs):
            opened.append(str(path))
            return real_open(path, flags, *args, **kwargs)

        with patch.object(probe.os, "open", record):
            result = probe.collect(str(self.root))
        self.assertEqual([f["path"] for f in result["files"]],
                         ["nested/ignored.py", "visible.py"])
        self.assertNotIn("visible.py", opened)
        self.assertNotIn("ignored.py", opened)
        self.assertNotIn("SOURCE_BODY_MUST_NOT_BE_TRANSFERRED", json.dumps(result))
        self.assertEqual(result["rules"][0]["text"], "ignored.py\n")
        self.assertFalse(result["policy_complete"])

    def test_file_directory_links_and_special_files_are_not_read(self):
        outside = self.root / "outside"
        outside.mkdir()
        (outside / "secret.py").write_text("OUTSIDE_SOURCE_MUST_NOT_BE_READ")
        workspace = self.root / "workspace"
        workspace.mkdir()
        (workspace / "linked.py").symlink_to(outside / "secret.py")
        (workspace / "linked-dir").symlink_to(outside, target_is_directory=True)
        os.mkfifo(workspace / "pipe")
        result = probe.collect(str(workspace))
        self.assertEqual(result["files"], [])
        self.assertEqual(result["rules"], [])
        self.assertEqual(result["skipped_links"], ["linked-dir", "linked.py"])

    def test_non_regular_ignore_files_fail_instead_of_silently_losing_rules(self):
        rule = self.root / ".cursorignore"
        os.mkfifo(rule)
        with self.assertRaises(probe.InventoryError):
            probe.collect(str(self.root))
        rule.unlink()
        rule.symlink_to(self.write("private.py"))
        with self.assertRaises(probe.InventoryError):
            probe.collect(str(self.root))

    def test_root_and_ancestor_symlinks_are_rejected(self):
        self.write("physical/nested/file.py")
        (self.root / "link").symlink_to(self.root / "physical", target_is_directory=True)
        for suffix in ("link", "link/nested"):
            with self.subTest(suffix=suffix), self.assertRaises(OSError):
                probe.collect(str(self.root / suffix))

    def test_bulk_and_unicode_names_have_no_display_budget_truncation(self):
        expected = []
        for i in range(160):
            name = f"bulk/file_{i:03d}_" + "x" * 45 + ".py"
            self.write(name)
            expected.append(name)
        self.write("中文🙂\nfile.py")
        expected.append("中文🙂\nfile.py")
        self.assertEqual([f["path"] for f in probe.collect(str(self.root))["files"]],
                         sorted(expected))

    def test_entry_output_and_rule_limits_return_no_partial_result(self):
        self.write("one.py")
        self.write("two.py")
        for name, value in (("MAX_ENTRIES", 1), ("MAX_OUTPUT_BYTES", 10),
                            ("MAX_SECONDS", -1)):
            with self.subTest(name=name), patch.object(probe, name, value):
                with self.assertRaises(probe.InventoryError):
                    probe.collect(str(self.root))
        self.write(".sembleignore", "x" * 20)
        for name in ("MAX_RULE_BYTES", "MAX_TOTAL_RULE_BYTES"):
            with self.subTest(name=name), patch.object(probe, name, 10):
                with self.assertRaises(probe.InventoryError):
                    probe.collect(str(self.root))

    def test_rule_swap_to_symlink_is_rejected_before_read(self):
        self.write("source.py")
        rule = self.write(".cursorignore", "source.py\n")
        real_open = os.open

        def swap(path, flags, *args, **kwargs):
            if path == ".cursorignore":
                rule.unlink()
                rule.symlink_to(self.root / "source.py")
            return real_open(path, flags, *args, **kwargs)

        with patch.object(probe.os, "open", swap), self.assertRaises(OSError):
            probe.collect(str(self.root))

    def test_non_utf8_rules_are_rejected(self):
        (self.root / ".cursorignore").write_bytes(b"\xff")
        with self.assertRaises(UnicodeError):
            probe.collect(str(self.root))

    def test_cli_launcher_returns_one_complete_json_object(self):
        self.write("visible.py")
        command = [sys.executable, str(Path(probe.__file__).resolve())]
        success = subprocess.run(command + [str(self.root)], capture_output=True, text=True)
        self.assertEqual(success.returncode, 0, success.stderr)
        self.assertEqual(json.loads(success.stdout)["root"], str(self.root))
        error = subprocess.run(command + [str(self.root / "missing")],
                               capture_output=True, text=True)
        self.assertEqual(error.returncode, 2)
        body = json.loads(error.stdout)
        self.assertFalse(body["ok"])
        self.assertNotIn("files", body)


if __name__ == "__main__":
    unittest.main()
