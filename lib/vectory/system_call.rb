require "open3"
require_relative "capture"

module Vectory
  class SystemCall
    TIMEOUT = 60

    attr_reader :status, :stdout, :stderr, :cmd

    def initialize(cmd, timeout = TIMEOUT)
      @cmd = cmd
      @timeout = timeout
    end

    def call
      log_cmd(@cmd)

      execute(@cmd)

      log_result

      raise_error unless @status.success?

      self
    end

    private

    def log_cmd(cmd)
      Vectory.ui.debug("Cmd: '#{cmd}'")
    end

    def execute(cmd)
      result = Capture.with_timeout(cmd,
                                    timeout: @timeout,
                                    signal: :KILL, # only KILL works on Windows
                                    kill_after: 2)
      @stdout = result[:stdout] || ""
      @stderr = result[:stderr] || ""
      @status = result[:status]
      @timed_out = result[:timeout]
    rescue Errno::ENOENT => e
      raise SystemCallError, e.inspect
    end

    def log_result
      Vectory.ui.debug("Status: #{@status.inspect}")
      Vectory.ui.debug("Stdout: '#{@stdout.strip}'")
      Vectory.ui.debug("Stderr: '#{@stderr.strip}'")
    end

    def raise_error
      if @timed_out
        raise SystemCallError,
              "Command timed out after #{@timeout} seconds: #{@cmd},\n  " \
              "stdout: '#{@stdout.strip}',\n  " \
              "stderr: '#{@stderr.strip}'"
      end

      if @status.nil?
        raise SystemCallError,
              "Failed to run #{@cmd} (no status available),\n  " \
              "stdout: '#{@stdout.strip}',\n  " \
              "stderr: '#{@stderr.strip}'"
      end

      raise SystemCallError,
            "Failed to run #{@cmd},\n  " \
            "status: #{@status.exitstatus},\n  " \
            "stdout: '#{@stdout.strip}',\n  " \
            "stderr: '#{@stderr.strip}'"
    end
  end
end
