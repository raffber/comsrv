pipeline {
    agent {
        dockerfile {
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
                sh './jenkins/build-comsrv.sh'
                archiveArtifacts(artifacts: 'comsrv/target/release/comsrv, comsrv/target/x86_64-pc-windows-msvc/release/comsrv.exe')
            }
        }
    }
}
