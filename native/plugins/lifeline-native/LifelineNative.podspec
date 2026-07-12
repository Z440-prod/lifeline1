require 'json'

package = JSON.parse(File.read(File.join(__dir__, 'package.json')))

Pod::Spec.new do |s|
  s.name = 'LifelineNative'
  s.version = package['version']
  s.summary = package['description']
  s.license = 'MIT'
  s.homepage = 'https://lifeline.health'
  s.author = 'Lifeline'
  s.source = { :git => 'https://lifeline.health', :tag => s.version.to_s }
  s.source_files = 'ios/Sources/**/*.{swift,h,m}'
  s.ios.deployment_target = '16.0'
  s.dependency 'Capacitor'
  # Sign in with Google (optional — remove if you only ship Apple sign-in):
  # s.dependency 'GoogleSignIn', '~> 7.0'
  # On-device AI (optional — MediaPipe LLM Inference for iOS):
  # s.dependency 'MediaPipeTasksGenAI'
  s.swift_version = '5.9'
end
