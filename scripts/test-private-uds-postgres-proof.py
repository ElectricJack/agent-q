#!/usr/bin/env python3
"""Lifecycle guards without Docker or database mutations."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("uds_proof", Path(__file__).with_name("private-uds-postgres-proof.py"))
proof = importlib.util.module_from_spec(spec)
spec.loader.exec_module(proof)


class CleanupOwnershipTests(unittest.TestCase):
    def test_promoted_and_broken_markers_refuse_before_container_access(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            marker = root / proof.PROMOTED_MARKER
            for broken in [False, True]:
                if broken:
                    marker.symlink_to(root / "missing")
                else:
                    marker.write_text("malformed promotion metadata")
                with patch.object(proof, "container") as container, patch.object(proof, "run") as run:
                    with self.assertRaisesRegex(RuntimeError, "promoted consumer"):
                        proof.stop(root, {})
                    container.assert_not_called()
                    run.assert_not_called()
                marker.unlink()

    def test_additional_database_refuses_destructive_container_commands(self):
        info = {"Id": "owned", "State": {"Running": True}}
        databases = ["postgres", "template0", "template1", "proof_db", "canonical_db"]
        with patch.object(proof, "container", return_value=info), patch.object(proof, "sql", return_value=json.dumps(databases)), patch.object(proof, "run") as run:
            with self.assertRaisesRegex(RuntimeError, "additional databases"):
                proof.stop(Path("/unused"), {"database": "proof_db"})
            run.assert_not_called()

    def test_isolated_owned_database_can_still_stop(self):
        info = {"Id": "owned", "State": {"Running": True}}
        databases = ["postgres", "template0", "template1", "proof_db"]
        data = {"database": "proof_db"}
        with patch.object(proof, "container", return_value=info), patch.object(proof, "sql", return_value=json.dumps(databases)), patch.object(proof, "run") as run, patch.object(proof, "save"), contextlib.redirect_stdout(io.StringIO()):
            proof.stop(Path("/unused"), data)
        self.assertEqual(run.call_args_list[0].args[0], ["docker", "stop", "--timeout=10", "owned"])
        self.assertEqual(run.call_args_list[1].args[0], ["docker", "rm", "owned"])
        self.assertEqual(data["state"], "stopped")


if __name__ == "__main__":
    unittest.main()
