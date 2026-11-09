# System Ruby, standard library only. Kept separate so statistics/tree logic can be tested
# without launching a window: bash scripts/bench.sh --test.
require 'json'
require 'fileutils'
require 'time'
require 'open3'

module Bench
  ELECTRON = '/Applications/Locust.app'
  MAIN = "#{ELECTRON}/Contents/MacOS/Locust"
  CPU_WARMUP_SECONDS = 10
  QUIT_MARGIN_SECONDS = 2
  def self.median(values)
    raise 'no samples' if values.empty?
    sorted = values.sort
    n = sorted.length
    (sorted[(n - 1) / 2] + sorted[n / 2]) / 2.0
  end
  def self.mean(values)
    raise 'no samples' if values.empty?
    values.sum / values.length.to_f
  end
  def self.tree(rows, roots)
    ids = roots.dup
    loop do
      added = rows.select { |r| ids.include?(r[:ppid]) && !ids.include?(r[:pid]) }.map { |r| r[:pid] }
      break if added.empty?
      ids.concat(added)
    end
    rows.select { |r| ids.include?(r[:pid]) }
  end
  def self.electron_roots(rows)
    # A detached crashpad handler may no longer descend from the main process.
    rows.select { |r| r[:command] == MAIN || r[:command].start_with?(MAIN + ' ') ||
      r[:command].start_with?(ELECTRON + '/Contents/Frameworks/') }.map { |r| r[:pid] }
  end
  def self.cpu_seconds(value)
    # BSD ps time: minutes:seconds, hours:minutes:seconds, optionally days-hours.
    match = /\A(?:(\d+)-)?(?:(\d+):)?(\d+):(\d+(?:\.\d+)?)\z/.match(value)
    raise "invalid ps CPU time: #{value}" unless match
    match[1].to_i * 86400 + match[2].to_i * 3600 + match[3].to_i * 60 + match[4].to_f
  end
  def self.parse_ps(output)
    output.lines.map do |line|
      fields = line.strip.split(/\s+/, 7)
      next unless fields.length == 7
      {pid: Integer(fields[0]), ppid: Integer(fields[1]), rss_kib: Integer(fields[2]),
       stat: fields[3], cpu_seconds: cpu_seconds(fields[4]),
       ps_cpu_percent_estimate: Float(fields[5]), command: fields[6]}
    end.compact
  end
  def self.ps
    output, status = Open3.capture2('/bin/ps', '-axo', 'pid=,ppid=,rss=,stat=,time=,%cpu=,command=')
    raise 'ps failed' unless status.success?
    parse_ps(output)
  end
  def self.live_rows(rows)
    rows.reject { |r| r[:stat].to_s.include?('Z') || r[:command].to_s.include?('<defunct>') || r[:rss_kib] <= 0 }
  end
  def self.native_rows(rows, pid)
    rows = rows.select { |r| r[:pid] == pid }
    raise 'native exited or has a defunct/zero-RSS row before sampling completed' if rows.empty? || live_rows(rows).length != rows.length
    rows
  end
  def self.sample(rows)
    {at: Time.now.utc.iso8601(6), monotonic_seconds: clock,
     rss_kib: rows.sum { |r| r[:rss_kib] },
     ps_cpu_percent_estimate: rows.sum { |r| r[:ps_cpu_percent_estimate] },
     processes: rows.map { |r| r.reject { |k, _| k == :command } }}
  end
  def self.interval_cpu(start_sample, end_sample)
    wall = end_sample.fetch(:monotonic_seconds) - start_sample.fetch(:monotonic_seconds)
    raise 'CPU interval must have positive wall duration' unless wall > 0
    first = start_sample.fetch(:processes).to_h { |r| [r[:pid], r[:cpu_seconds]] }
    last = end_sample.fetch(:processes).to_h { |r| [r[:pid], r[:cpu_seconds]] }
    common = first.keys & last.keys
    raise 'no PIDs present at both CPU interval endpoints' if common.empty?
    deltas = common.map do |pid|
      delta = last.fetch(pid) - first.fetch(pid)
      raise "CPU counter decreased for PID #{pid}; possible PID reuse" if delta < 0
      delta
    end
    {cpu_percent: deltas.sum / wall * 100.0, cpu_seconds: deltas.sum, wall_seconds: wall,
     counted_pids: common.sort, start_only_pids: (first.keys - last.keys).sort,
     end_only_pids: (last.keys - first.keys).sort,
     start: start_sample, end: end_sample}
  end
  def self.summary(samples)
    {peak_rss_kib: samples.map { |s| s[:rss_kib] }.max,
     median_idle_rss_kib: median(samples.map { |s| s[:rss_kib] })}
  end
  def self.du(path)
    return nil unless File.exist?(path)
    output, status = Open3.capture2('/usr/bin/du', '-sk', path)
    raise "du failed: #{path}" unless status.success?
    output.split.first.to_i * 1024
  end
  def self.capture(*args)
    output, status = Open3.capture2(*args)
    raise "command failed: #{args.first}" unless status.success?
    output.strip
  end
  def self.clock
    Process.clock_gettime(Process::CLOCK_MONOTONIC)
  end
  def self.native_run(binary, out, seconds, index)
    frame = File.join(out, "native-#{index}-frame.json")
    FileUtils.rm_f(frame)
    quit_seconds = CPU_WARMUP_SECONDS + seconds + QUIT_MARGIN_SECONDS
    pid = Process.spawn(binary, '--bench', frame, '--bench-seconds', quit_seconds.to_s,
                        out: File.join(out, "native-#{index}.log"), err: [:child, :out])
    reaped = false
    begin
      deadline = clock + 60
      until File.file?(frame) && (JSON.parse(File.read(frame)) rescue nil)
        if Process.waitpid(pid, Process::WNOHANG)
          reaped = true
          raise "native exited before first frame; inspect native-#{index}.log"
        end
        raise 'no real first frame within 60 seconds (possibly fully occluded)' if clock >= deadline
        sleep 0.05
      end
      first_frame = JSON.parse(File.read(frame)).fetch('first_frame_ms')
      samples = []
      cpu_start = nil
      cpu_end = nil
      sampling_start = clock
      # RSS retains its original time-zero window. CPU has a separate 10 s warmup.
      (CPU_WARMUP_SECONDS + seconds + 1).times do |i|
        delay = sampling_start + i - clock
        sleep delay if delay > 0
        if Process.waitpid(pid, Process::WNOHANG)
          reaped = true
          raise "native exited before sampling completed; inspect native-#{index}.log"
        end
        rows = native_rows(ps, pid)
        # Also close the race where the process exits during the ps invocation.
        if Process.waitpid(pid, Process::WNOHANG)
          reaped = true
          raise "native exited during sampling; inspect native-#{index}.log"
        end
        snapshot = sample(rows)
        samples << snapshot if i < seconds
        cpu_start = snapshot if i == CPU_WARMUP_SECONDS
        cpu_end = snapshot if i == CPU_WARMUP_SECONDS + seconds
      end
      cpu = interval_cpu(cpu_start, cpu_end)
      deadline = clock + 10
      until (done = Process.waitpid(pid, Process::WNOHANG))
        raise 'native did not quit after idle window' if clock >= deadline
        sleep 0.05
      end
      reaped = true
      raise 'native exited unsuccessfully' unless $?.success?
      {first_frame_ms: first_frame, samples: samples, cpu_interval: cpu,
       summary: summary(samples).merge(first_frame_ms: first_frame, idle_cpu_percent: cpu[:cpu_percent])}
    ensure
      unless reaped
        Process.kill('TERM', pid) rescue Errno::ESRCH
        Process.waitpid(pid) rescue Errno::ECHILD
      end
    end
  end
  def self.run(out, here)
    seconds = Integer(ENV.fetch('BENCH_SECONDS', '30'))
    raise 'BENCH_SECONDS must be a positive integer' unless seconds > 0
    target = File.expand_path(ENV.fetch('CARGO_TARGET_DIR', File.join(here, 'target')), here)
    binary = File.join(target, 'release/locust-native')
    raise "release binary missing: #{binary}; build it first or set CARGO_TARGET_DIR" unless File.executable?(binary)
    FileUtils.mkdir_p(out)
    # Reject accidental reuse so prior raw samples remain inspectable.
    raise "output already contains a benchmark: #{out}" if File.exist?(File.join(out, 'samples.json'))
    raw = {date: Time.now.utc.iso8601, macos: capture('/usr/bin/sw_vers', '-productVersion'),
           chip: capture('/usr/sbin/sysctl', '-n', 'machdep.cpu.brand_string'), idle_seconds: seconds,
           native_cpu_warmup_seconds: CPU_WARMUP_SECONDS, native_quit_margin_seconds: QUIT_MARGIN_SECONDS,
           native_version: File.read(File.join(here, 'Cargo.toml'))[/^version = "([^"]+)"/, 1],
           native_binary: binary, native_binary_bytes: File.size(binary), native_bundle_bytes: du(File.join(target, 'Locust Native.app')),
           electron_bundle_bytes: du(ELECTRON)}
    plist = File.join(ELECTRON, 'Contents/Info.plist')
    raw[:electron_version] = File.file?(plist) ? capture('/usr/libexec/PlistBuddy', '-c', 'Print :CFBundleShortVersionString', plist) : nil
    raw[:native_runs] = (1..3).map do |i|
      warn "Native run #{i}/3 (#{seconds}s RSS; CPU #{seconds}s after #{CPU_WARMUP_SECONDS}s warmup)"
      native_run(binary, out, seconds, i)
    end
    native = raw[:native_runs].first[:summary].keys.to_h { |key| [key, median(raw[:native_runs].map { |r| r[:summary][key] })] }
    raw[:native_medians] = native
    rows = ps
    running = rows.any? { |r| r[:command] == MAIN || r[:command].start_with?(MAIN + ' ') }
    electron = nil
    if running
      samples = []
      cpu_start = nil
      cpu_end = nil
      sampling_start = clock
      (seconds + 1).times do |i|
        delay = sampling_start + i - clock
        sleep delay if delay > 0
        rows = ps
        break unless live_rows(rows).any? { |r| r[:command] == MAIN || r[:command].start_with?(MAIN + ' ') }
        group = live_rows(tree(rows, electron_roots(rows)))
        break if group.empty?
        snapshot = sample(group)
        samples << snapshot if i < seconds
        cpu_start = snapshot if i == 0
        cpu_end = snapshot if i == seconds
      end
      if cpu_start && cpu_end
        cpu = interval_cpu(cpu_start, cpu_end)
        electron = summary(samples).merge(idle_cpu_percent: cpu[:cpu_percent])
        raw[:electron] = {samples: samples, cpu_interval: cpu, summary: electron}
      else
        raw[:electron] = {samples: samples, status: 'exited during sampling; comparison skipped'}
      end
    else
      raw[:electron] = {status: 'not running; skipped'}
    end
    notes = [
      'Native values are medians of three runs; Electron uses one existing-instance idle window.',
      'First frame: elapsed from entry to main to Rune’s post-present hook (after first-frame GPU completion), not physical display scanout or OS process creation.',
      'Electron cold start is unavailable: the working app is never launched, quit, or modified.',
      'Electron holds real conversations; native uses synthetic data. This favors native somewhat in the memory comparison.',
      'Idle CPU is the sum of cumulative ps time deltas divided by measured monotonic wall seconds × 100. Only PIDs present at both interval endpoints count; processes appearing or disappearing in the window are excluded (PID lists and endpoints are in samples.json). This can undercount process churn. ps time has limited precision; ps %CPU is retained only as a labelled estimate in raw samples.',
      'Native CPU starts 10 seconds after first-frame output and lasts the requested idle duration. Native quit is delayed by that warmup plus a two-second margin beyond the measured interval. Electron CPU covers the existing-instance idle window without a startup warmup.',
      'RSS is summed across Electron main, descendants, Helpers and detached crashpad; shared pages can be counted more than once.',
      'RSS samples start after native first-frame output; one sample per second, including time zero, with peak/median calculated as before. Early native exit or a defunct/zero-RSS row fails the run. A fully occluded native window may fail to produce a frame; the script fails rather than inventing a startup time.'
    ]
    raw[:notes] = notes
    File.write(File.join(out, 'samples.json'), JSON.pretty_generate(raw) + "\n")
    fmt = ->(v, unit) { v ? format('%.2f %s', v, unit) : 'unavailable' }
    mib = ->(bytes) { bytes && bytes / 1048576.0 }
    lines = ["# Locust benchmark — #{raw[:date]}", '',
      "macOS #{raw[:macos]} · #{raw[:chip]} · native #{raw[:native_version]} · Electron #{raw[:electron_version] || 'unavailable'} · #{seconds}s idle window", '',
      '| metric | native | Electron |', '| --- | ---: | ---: |',
      "| Release binary | #{fmt.call(mib.call(raw[:native_binary_bytes]), 'MiB')} | — |",
      "| App bundle (du allocated) | #{fmt.call(mib.call(raw[:native_bundle_bytes]), 'MiB')} | #{fmt.call(mib.call(raw[:electron_bundle_bytes]), 'MiB')} |",
      "| First frame | #{fmt.call(native[:first_frame_ms], 'ms')} | unavailable (no relaunch) |"]
    [[:peak_rss_kib, 'Peak RSS', 'MiB', 1024.0], [:median_idle_rss_kib, 'Median idle RSS', 'MiB', 1024.0], [:idle_cpu_percent, 'Idle CPU (interval)', '%', 1.0]].each do |key, label, unit, divisor|
      lines << "| #{label} | #{fmt.call(native[key] / divisor, unit)} | #{electron ? fmt.call(electron[key] / divisor, unit) : 'skipped'} |"
    end
    lines += ['', "Electron: #{raw[:electron][:status] || 'already running; measured without controlling it'}.", ''] + notes.map { |n| "- #{n}" }
    report = lines.join("\n") + "\n"
    File.write(File.join(out, 'report.md'), report)
    puts report
  end
end

if ARGV.first == '--test'
  ARGV.shift
  require 'minitest/autorun'
  class BenchTest < Minitest::Test
    def test_statistics
      assert_equal 3.0, Bench.median([9, 1, 3])
      assert_equal 2.5, Bench.median([4, 1, 3, 2])
      assert_equal 2.0, Bench.mean([1, 2, 3])
      assert_equal 7.0, Bench.median([7])
      assert_raises(RuntimeError) { Bench.median([]) }
      assert_raises(RuntimeError) { Bench.mean([]) }
    end
    def test_defunct_and_zero_rss_rows_are_not_native_samples
      rows = Bench.parse_ps("42 1 2048 S 0:01.25 0.5 /native\n43 1 0 Z 0:00.00 0.0 <defunct>\n44 1 0 S 0:00.00 0.0 /native\n45 1 100 S 0:00.00 0.0 <defunct>\n46 1 100 Z+ 0:00.00 0.0 /native\n")
      assert_equal [42], Bench.live_rows(rows).map { |r| r[:pid] }
      assert_equal [rows.first], Bench.native_rows(rows, 42)
      [43, 44, 45, 46, 999].each { |pid| assert_raises(RuntimeError) { Bench.native_rows(rows, pid) } }
      assert_equal 1.25, rows.first[:cpu_seconds]
    end
    def test_cpu_time_formats
      assert_in_delta 754.56, Bench.cpu_seconds('12:34.56'), 0.0001
      assert_in_delta 3723.45, Bench.cpu_seconds('1:02:03.45'), 0.0001
      assert_in_delta 93784.5, Bench.cpu_seconds('1-02:03:04.50'), 0.0001
      assert_equal 0, Bench.cpu_seconds('0:00.00')
      ['bad', '-1:00', '1', '0:NaN'].each { |value| assert_raises(RuntimeError) { Bench.cpu_seconds(value) } }
    end
    def test_interval_cpu_excludes_pid_churn_and_startup_cpu
      snapshot = ->(time, counters) { {monotonic_seconds: time,
        processes: counters.map { |pid, cpu| {pid: pid, cpu_seconds: cpu} }} }
      first = snapshot.call(10.0, {1 => 20.0, 2 => 4.0, 3 => 100.0})
      last = snapshot.call(40.0, {1 => 20.1, 2 => 4.2, 4 => 200.0})
      cpu = Bench.interval_cpu(first, last)
      assert_in_delta 1.0, cpu[:cpu_percent], 0.0001
      assert_in_delta 0.3, cpu[:cpu_seconds], 0.0001
      assert_equal 30.0, cpu[:wall_seconds]
      assert_equal [1, 2], cpu[:counted_pids]
      assert_equal [3], cpu[:start_only_pids]
      assert_equal [4], cpu[:end_only_pids]
      assert_equal 0.0, Bench.interval_cpu(first, snapshot.call(20.0, {1 => 20.0}))[:cpu_percent]
      assert_raises(RuntimeError) { Bench.interval_cpu(first, first) }
      assert_raises(RuntimeError) { Bench.interval_cpu(first, snapshot.call(20.0, {4 => 1.0})) }
      assert_raises(RuntimeError) { Bench.interval_cpu(first, snapshot.call(20.0, {1 => 19.0})) }
    end
    def test_tree_and_detached_helpers
      row = ->(pid, ppid, command) { {pid: pid, ppid: ppid, command: command, rss_kib: 10, stat: 'S', cpu_seconds: 1.0, ps_cpu_percent_estimate: 0.5} }
      rows = [row.call(1, 0, Bench::MAIN), row.call(2, 1, 'child'), row.call(3, 2, 'grandchild'),
              row.call(4, 0, Bench::ELECTRON + '/Contents/Frameworks/crashpad_handler'),
              row.call(5, 0, '/Applications/Other.app/Helper'), row.call(6, 0, Bench::MAIN + '-other')]
      selected = Bench.tree(rows.reverse, Bench.electron_roots(rows))
      assert_equal [1, 2, 3, 4], selected.map { |r| r[:pid] }.sort
      assert_equal 40, Bench.sample(selected)[:rss_kib]
      assert_equal 2.0, Bench.sample(selected)[:ps_cpu_percent_estimate]
      assert_equal [], Bench.tree(rows, [])
      assert_equal 10, Bench.summary([Bench.sample([rows.first])])[:peak_rss_kib]
    end
  end
elsif $PROGRAM_NAME == __FILE__
  begin
    Bench.run(File.expand_path(ARGV.fetch(0)), ARGV.fetch(1))
  rescue StandardError => e
    warn "Benchmark failed: #{e.message}"
    exit 1
  end
end
