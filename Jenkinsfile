pipeline {
    agent {
        docker {
            image 'docker.gcteu.ch/comsrv-agent'
        }
    }
    stages {
        stage('test-comsrv') {
            steps {
                sh 'cd comsrv && cargo test'
            }
        }
        stage('build-artifacts') {
            steps {
                sh 'cd comsrv && cargo build --release'
                sh 'cd comsrv && cargo xwin build --target x86_64-pc-windows-msvc --release'
                archiveArtifacts(artifacts: 'comsrv/target/release/comsrv, comsrv/target/x86_64-pc-windows-msvc/release/comsrv.exe')
            }
        }
    }
}
