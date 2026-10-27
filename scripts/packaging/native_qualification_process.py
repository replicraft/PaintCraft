"""Bounded serial qualification processes; this is not a product executor."""
import json
import os
from pathlib import Path
import signal
import subprocess
import threading
import time


class Runner:
    def __init__(self, output, *, limit=16 << 20, total_limit=128 << 20):
        self.output = Path(output)
        self.output.mkdir(parents=True, exist_ok=True)
        self.limit, self.total_limit = limit, total_limit
        self.total = 0
        self.deadline = time.monotonic() + 75 * 60
        self.steps = []
        self.lock = threading.Lock()

    def save(self):
        (self.output / "processes.json").write_text(json.dumps(self.steps, indent=2) + "\n", encoding="utf-8")

    @staticmethod
    def kill(process):
        errors = []
        try:
            if os.name == "nt":
                result = subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"],
                                        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                        timeout=15, check=False)
                if result.returncode and process.poll() is None:
                    errors.append("taskkill did not retire process tree")
            else:
                os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        except (OSError, subprocess.SubprocessError) as error:
            errors.append(f"Process tree cleanup: {error}")
        if process.poll() is None:
            try:
                process.kill()
            except ProcessLookupError:
                pass
            except OSError as error:
                errors.append(f"Process cleanup: {error}")
        return errors

    def run(self, name, argv, *, cwd, env, timeout=900, allow_failure=False):
        if not name.replace("-", "").isalnum() or not 0 < timeout <= 3600:
            raise ValueError("Invalid bounded qualification step")
        remaining = self.deadline - time.monotonic()
        if remaining <= 0:
            raise RuntimeError("Qualification total 75-minute budget exhausted")
        timeout = min(timeout, remaining)
        index = len(self.steps)
        stdout = self.output / f"{index:02d}-{name}.stdout"
        stderr = self.output / f"{index:02d}-{name}.stderr"
        row = {"name": name, "argv": list(map(str, argv)), "timeout_seconds": timeout,
               "started_epoch": time.time(), "stdout": stdout.name, "stderr": stderr.name,
               "bytes": 0, "timeout": False, "output_limit": False}
        self.steps.append(row)
        self.save()
        print(f"START {name}", flush=True)
        options = ({"creationflags": subprocess.CREATE_NEW_PROCESS_GROUP} if os.name == "nt"
                   else {"start_new_session": True})
        try:
            process = subprocess.Popen(row["argv"], cwd=cwd, env=env,
                                       stdout=subprocess.PIPE, stderr=subprocess.PIPE, **options)
        except OSError as error:
            row.update(error=str(error), finished_epoch=time.time())
            self.save()
            raise
        row["pid"] = process.pid
        faults = []

        def read(stream, path):
            try:
                with path.open("xb") as log:
                    while chunk := stream.read(65536):
                        with self.lock:
                            available = min(self.limit - row["bytes"], self.total_limit - self.total)
                            saved = chunk[:max(0, available)]
                            row["bytes"] += len(saved)
                            self.total += len(saved)
                            log.write(saved)
                            exceeded = len(saved) != len(chunk)
                            row["output_limit"] |= exceeded
                        if exceeded:
                            faults.extend(self.kill(process))
                            break
            except OSError as error:
                faults.append(str(error))
                faults.extend(self.kill(process))
            finally:
                stream.close()

        threads = [threading.Thread(target=read, args=(process.stdout, stdout), daemon=True),
                   threading.Thread(target=read, args=(process.stderr, stderr), daemon=True)]
        for thread in threads:
            thread.start()
        try:
            process.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            row["timeout"] = True
            faults.extend(self.kill(process))
            process.wait(timeout=20)
        finally:
            for thread in threads:
                thread.join(timeout=3)
            if any(thread.is_alive() for thread in threads):
                faults.extend(self.kill(process))
                for thread in threads:
                    thread.join(timeout=3)
                faults.append("Output pipe did not retire with process tree")
            row.update(exit=process.returncode, finished_epoch=time.time(), transport_faults=faults)
            self.save()
        print(f"TERMINAL {name} exit={row['exit']}", flush=True)
        if ((row["exit"] != 0 and not allow_failure) or row["timeout"]
                or row["output_limit"] or faults):
            raise RuntimeError(f"Qualification step {name} failed; raw output retained")
        return stdout
