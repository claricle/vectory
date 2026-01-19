require "timeout"

module Vectory
  module Capture
    class << self
      def windows?
        !!((RUBY_PLATFORM =~ /(win|w)(32|64)$/) ||
           (RUBY_PLATFORM =~ /mswin|mingw/))
      end

      # Capture the standard output and the standard error of a command.
      # Almost same as Open3.capture3 method except for timeout handling and return value.
      # See Open3.capture3.
      #
      #   result = with_timeout([env,] cmd... [, opts])
      #
      # The arguments env, cmd and opts are passed to Process.spawn except
      # opts[:stdin_data], opts[:binmode], opts[:timeout], opts[:signal]
      # and opts[:kill_after].  See Process.spawn.
      #
      # If opts[:stdin_data] is specified, it is sent to the command's standard input.
      #
      # If opts[:binmode] is true, internal pipes are set to binary mode.
      #
      # If opts[:timeout] is specified, SIGTERM is sent to the command after specified seconds.
      #
      # If opts[:signal] is specified, it is used instead of SIGTERM on timeout.
      #
      # If opts[:kill_after] is specified, also send a SIGKILL after specified seconds.
      # it is only sent if the command is still running after the initial signal was sent.
      #
      # The return value is a Hash as shown below.
      #
      #   {
      #     :pid     => PID of the command,
      #     :status  => Process::Status of the command,
      #     :stdout  => the standard output of the command,
      #     :stderr  => the standard error of the command,
      #     :timeout => whether the command was timed out,
      #   }
      def with_timeout(*cmd)
        spawn_opts = Hash === cmd.last ? cmd.pop.dup : {}

        # Separate environment variables (string keys) from spawn options (symbol keys)
        env_vars = spawn_opts.reject { |k, _| k.is_a?(Symbol) }
        spawn_opts = spawn_opts.reject { |k, _| k.is_a?(String) }

        # Windows only supports :KILL signal reliably, Unix can use :TERM for graceful shutdown
        default_signal = windows? ? :KILL : :TERM
        opts = {
          stdin_data: spawn_opts.delete(:stdin_data) || "",
          binmode: spawn_opts.delete(:binmode) || false,
          timeout: spawn_opts.delete(:timeout),
          signal: spawn_opts.delete(:signal) || default_signal,
          kill_after: spawn_opts.delete(:kill_after) || 2,
        }

        in_r,  in_w  = IO.pipe
        out_r, out_w = IO.pipe
        err_r, err_w = IO.pipe
        in_w.sync = true

        if opts[:binmode]
          in_w.binmode
          out_r.binmode
          err_r.binmode
        end

        spawn_opts[:in]  = in_r
        spawn_opts[:out] = out_w
        spawn_opts[:err] = err_w

        result = {
          pid: nil,
          status: nil,
          stdout: "",
          stderr: "",
          timeout: false,
        }

        out_reader = nil
        err_reader = nil
        wait_thr = nil
        watchdog = nil

        begin
          # Pass environment variables and command to spawn
          # spawn signature: spawn([env], cmd..., [options])
          # If env_vars is not empty, pass it as the first argument
          result[:pid] = if env_vars.any?
                           spawn(env_vars, *cmd, spawn_opts)
                         else
                           spawn(*cmd, spawn_opts)
                         end
          wait_thr = Process.detach(result[:pid])
          in_r.close
          out_w.close
          err_w.close

          # Start reader threads with timeout protection
          out_reader = Thread.new do
            out_r.read
          rescue StandardError => e
            Vectory.ui.debug("Output reader error: #{e}")
            ""
          end

          err_reader = Thread.new do
            err_r.read
          rescue StandardError => e
            Vectory.ui.debug("Error reader error: #{e}")
            ""
          end

          # Write input data
          begin
            in_w.write opts[:stdin_data]
            in_w.close
          rescue Errno::EPIPE
            # Process may have exited early
          end

          # Watchdog thread to enforce timeout
          if opts[:timeout]
            watchdog = Thread.new do
              sleep opts[:timeout]
              if windows?
                # Windows: Use spawn to run taskkill in background (non-blocking)
                if wait_thr.alive?
                  result[:timeout] = true
                  # Spawn taskkill in background to avoid blocking
                  begin
                    Process.spawn("taskkill", "/pid", result[:pid].to_s, "/f",
                                  %i[out err] => File::NULL)
                  rescue Errno::ENOENT
                    # taskkill not found (shouldn't happen on Windows)
                  end
                end
              elsif wait_thr.alive?
                # Unix: Use Process.kill which works reliably
                result[:timeout] = true
                pid = spawn_opts[:pgroup] ? -result[:pid] : result[:pid]

                begin
                  Process.kill(opts[:signal], pid)
                rescue Errno::ESRCH, Errno::EINVAL, Errno::EPERM
                  # Process already dead, invalid signal, or permission denied
                end

                # Wait for kill_after duration, then force kill
                sleep opts[:kill_after]
                if wait_thr.alive?
                  begin
                    Process.kill(:KILL, pid)
                  rescue Errno::ESRCH, Errno::EINVAL, Errno::EPERM
                    # Process already dead, invalid signal, or permission denied
                  end
                end
              end
            end
          end

          # Wait for process to complete with timeout
          if opts[:timeout]
            if windows?
              # On Windows, use polling with timeout to avoid long sleeps
              deadline = Time.now + opts[:timeout] + 5
              loop do
                break unless wait_thr.alive?
                break if Time.now > deadline

                sleep 0.5
              end
            else
              deadline = Time.now + opts[:timeout] + (opts[:kill_after] || 2) + 1
              loop do
                break unless wait_thr.alive?
                break if Time.now > deadline

                sleep 0.1
              end

              # Force kill if still alive after deadline
              if wait_thr.alive?
                begin
                  Process.kill(:KILL, result[:pid])
                rescue Errno::ESRCH, Errno::EINVAL, Errno::EPERM
                  # Process already dead, invalid signal, or permission denied
                end
              end
            end
          end

          # Wait for process status (with timeout protection)
          status_deadline = Time.now + 5
          while wait_thr.alive? && Time.now < status_deadline
            sleep 0.1
          end
        ensure
          # Clean up watchdog
          watchdog&.kill

          # Get process status
          begin
            result[:status] = wait_thr.value if wait_thr
          rescue StandardError => e
            Vectory.ui.debug("Error getting process status: #{e}")
            # Create a fake failed status
            result[:status] = Process::Status.allocate
          end

          # Get output with timeout protection
          if out_reader
            if out_reader.join(2)
              result[:stdout] = out_reader.value || ""
            else
              out_reader.kill
              result[:stdout] = ""
            end
          end

          if err_reader
            if err_reader.join(2)
              result[:stderr] = err_reader.value || ""
            else
              err_reader.kill
              result[:stderr] = ""
            end
          end

          # Close all pipes
          [in_w, out_r, err_r].each do |io|
            io.close unless io.closed?
          rescue StandardError => e
            Vectory.ui.debug("Error closing pipe: #{e}")
          end
        end

        result
      end
    end
  end
end
